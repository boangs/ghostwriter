use super::LLMEngine;
use crate::util::{option_or_env, option_or_env_fallback, OptionMap};
use anyhow::Result;
use log::{debug, info};
use serde_json::json;
use serde_json::Value as json;

use ureq::Error;

pub struct Tool {
    name: String,
    definition: json,
    callback: Option<Box<dyn FnMut(json)>>,
}

pub struct OpenAI {
    model: String,
    base_url: String,
    api_key: String,
    tools: Vec<Tool>,
    content: Vec<json>,
}

impl OpenAI {
    fn openai_tool_definition(tool: &Tool) -> json {
        json!({
                "type": "function",
                "function": {
            "name": tool.definition["name"],
            "description": tool.definition["description"],
            "parameters": tool.definition["parameters"],
                }
        })
    }

    pub fn add_content(&mut self, content: json) {
        self.content.push(content);
    }
}

impl LLMEngine for OpenAI {
    fn new(options: &OptionMap) -> Self {
        let api_key = option_or_env(&options, "api_key", "OPENAI_API_KEY");
        let base_url = option_or_env_fallback(
            &options,
            "base_url",
            "OPENAI_BASE_URL",
            "https://api.openai.com",
        );
        let model = options.get("model").unwrap().to_string();

        Self {
            model,
            base_url,
            api_key,
            tools: Vec::new(),
            content: Vec::new(),
        }
    }

    fn register_tool(&mut self, name: &str, definition: json, callback: Box<dyn FnMut(json)>) {
        self.tools.push(Tool {
            name: name.to_string(),
            definition,
            callback: Some(callback),
        });
    }

    fn add_text_content(&mut self, text: &str) {
        self.add_content(json!({
            "type": "text",
            "text": text,
        }));
    }

    fn add_image_content(&mut self, base64_image: &str) {
        self.add_content(json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:image/png;base64,{}", base64_image)
            }
        }));
    }

    fn clear_content(&mut self) {
        self.content.clear();
    }

    fn execute(&mut self) -> Result<()> {
        let body = json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": self.content
            }],
            "tools": self.tools.iter().map(|tool| Self::openai_tool_definition(tool)).collect::<Vec<_>>(),
            "tool_choice": "required",
            "parallel_tool_calls": false
        });

        debug!("Request: {}", body);
        
        // 根据 base_url 判断是哪种 API
        let api_url = if self.base_url.contains("localhost") || self.base_url.contains("192.168.1.170") {
            // Ollama API
            format!("{}/api/chat", self.base_url)
        } else if self.base_url.contains("volcengine.com") || self.base_url.contains("volces.com") {
            // 火山引擎 API V3
            format!("{}/api/v3/chat/completions", self.base_url)
        } else if self.base_url.contains("dashscope.aliyuncs.com") {
            // 千问 API 兼容模式
            format!("{}/compatible-mode/v1/chat/completions", self.base_url)
        } else {
            // OpenAI API
            format!("{}/v1/chat/completions", self.base_url)
        };

        let mut request = ureq::post(&api_url)
            .set("Content-Type", "application/json");

        // 根据不同的 API 设置不同的认证头
        if self.base_url.contains("volcengine.com") || self.base_url.contains("volces.com") {
            request = request.set("Authorization", &format!("Bearer {}", self.api_key));
        } else if self.base_url.contains("dashscope.aliyuncs.com") {
            // 千问 API 使用 Bearer 认证
            request = request.set("Authorization", &format!("Bearer {}", self.api_key));
        } else {
            request = request.set("Authorization", &format!("Bearer {}", self.api_key));
        }

        let raw_response = request.send_json(&body);

        let response = match raw_response {
            Ok(response) => response,
            Err(Error::Status(code, response)) => {
                info!("Error: {}", code);
                let json: json = response.into_json()?;
                debug!("Response: {}", json);
                return Err(anyhow::anyhow!("API ERROR: {}", json));
            }
            Err(e) => return Err(anyhow::anyhow!("OTHER API ERROR: {}", e)),
        };

        let json: json = response.into_json().unwrap();
        debug!("Response: {}", json);

        // 处理不同 API 的响应格式
        let tool_calls = if self.base_url.contains("localhost") || self.base_url.contains("127.0.0.1") {
            // Ollama 格式
            &json["message"]["tool_calls"]
        } else if self.base_url.contains("volcengine.com") {
            // 火山引擎格式 (与 OpenAI 相同)
            &json["choices"][0]["message"]["tool_calls"]
        } else if self.base_url.contains("dashscope.aliyuncs.com") {
            // 千问兼容模式格式 (与 OpenAI 相同)
            &json["choices"][0]["message"]["tool_calls"]
        } else {
            // OpenAI 格式
            &json["choices"][0]["message"]["tool_calls"]
        };

        if let Some(tool_call) = tool_calls.get(0) {
            let function_name = tool_call["function"]["name"].as_str().unwrap();
            let function_input_raw = tool_call["function"]["arguments"].as_str().unwrap();
            let function_input = serde_json::from_str::<json>(function_input_raw).unwrap();
            let tool = self
                .tools
                .iter_mut()
                .find(|tool| tool.name == function_name);

            if let Some(tool) = tool {
                if let Some(callback) = &mut tool.callback {
                    callback(function_input.clone());
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "No callback registered for tool {}",
                        function_name
                    ))
                }
            } else {
                Err(anyhow::anyhow!(
                    "No tool registered with name {}",
                    function_name
                ))
            }
        } else {
            Err(anyhow::anyhow!("No tool calls found in response"))
        }
    }
}