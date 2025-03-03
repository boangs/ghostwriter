use super::LLMEngine;
use crate::util::{option_or_env, option_or_env_fallback, OptionMap};
use anyhow::Result;
use log::{debug, info, error};
use serde_json::json;
use serde_json::Value as json;

use ureq::Error;
use reqwest::Client;
use std::time::Duration;

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
    client: Client,
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

    pub fn new(options: &OptionMap) -> Self {
        let api_key = option_or_env(&options, "api_key", "OPENAI_API_KEY");
        let base_url = option_or_env_fallback(
            &options,
            "base_url",
            "OPENAI_BASE_URL",
            "https://api.openai.com",
        );
        let model = options.get("model").unwrap().to_string();

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap();

        Self {
            model,
            base_url,
            api_key,
            tools: Vec::new(),
            content: Vec::new(),
            client,
        }
    }

    pub async fn chat(&self, messages: Vec<Message>) -> Result<String> {
        let messages_json: Vec<Value> = messages
            .into_iter()
            .map(|msg| {
                match msg {
                    Message::Text { role, content } => {
                        json!({
                            "role": match role {
                                Role::System => "system",
                                Role::User => "user",
                                Role::Assistant => "assistant",
                            },
                            "content": content,
                        })
                    }
                    Message::Image { role, content, image_data } => {
                        json!({
                            "role": match role {
                                Role::System => "system",
                                Role::User => "user",
                                Role::Assistant => "assistant",
                            },
                            "content": [
                                {
                                    "type": "text",
                                    "text": content
                                },
                                {
                                    "type": "image_url",
                                    "image_url": {
                                        "url": format!("data:image/png;base64,{}", image_data)
                                    }
                                }
                            ]
                        })
                    }
                }
            })
            .collect();

        let request_body = json!({
            "model": self.model,
            "messages": messages_json,
            "max_tokens": 4096
        });

        // 输出完整的请求信息
        info!("API 请求 URL: {}/v1/chat/completions", self.base_url);
        info!("请求头: Authorization: Bearer {}", self.api_key);
        info!("请求体: {}", serde_json::to_string_pretty(&request_body)?);

        let response = self.client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        
        info!("API 响应状态码: {}", status);
        info!("API 响应内容: {}", response_text);

        if !status.is_success() {
            error!("API 调用失败: {} - {}", status, response_text);
            return Err(anyhow::anyhow!("Error: {}", status));
        }

        let response_json: Value = serde_json::from_str(&response_text)?;
        
        Ok(response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }
}

impl LLMEngine for OpenAI {
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

        // print body for debugging
        debug!("Request: {}", body);
        let raw_response = ureq::post(format!("{}/v1/chat/completions", self.base_url).as_str())
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_json(&body);

        let response = match raw_response {
            Ok(response) => response,
            Err(Error::Status(code, response)) => {
                info!("Error: {}", code);
                let json: json = response.into_json()?;
                debug!("Response: {}", json);
                return Err(anyhow::anyhow!("API ERROR"));
            }
            Err(_) => return Err(anyhow::anyhow!("OTHER API ERROR")),
        };

        let json: json = response.into_json().unwrap();
        debug!("Response: {}", json);

        let tool_calls = &json["choices"][0]["message"]["tool_calls"];

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