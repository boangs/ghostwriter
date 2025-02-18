use super::LLMEngine;
use crate::util::{option_or_env, option_or_env_fallback, OptionMap};
use anyhow::Result;
use log::{debug, info};
use serde_json::json;
use serde_json::Value as json;

use ureq::Error;
use reqwest::blocking::Client;

pub struct Tool {
    name: String,
    definition: json,
    callback: Option<Box<dyn FnMut(json)>>,
}

pub struct Google {
    model: String,
    base_url: String,
    api_key: String,
    tools: Vec<Tool>,
    content: Vec<json>,
}

impl Google {
    fn google_tool_definition(tool: &Tool) -> json {
        json!({
            "name": tool.definition["name"],
            "description": tool.definition["description"],
            "parameters": tool.definition["parameters"],
        })
    }

    pub fn add_content(&mut self, content: json) {
        self.content.push(content);
    }

    fn build_request(&self) -> Result<serde_json::Value> {
        let mut messages = Vec::new();
        messages.push(json!({
            "role": "user",
            "content": self.content
        }));

        Ok(json!({
            "model": self.model,
            "messages": messages
        }))
    }

    fn send_request(&self, request: &serde_json::Value) -> Result<reqwest::blocking::Response> {
        let client = Client::new();
        let response = client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(request)
            .send()?;
            
        Ok(response)
    }

    fn execute(&mut self) -> Result<String> {
        info!("执行 Google LLM 引擎");
        
        // 构建请求体
        let body = json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": &self.content
            }]
        });

        // 发送请求
        let response = ureq::post(&format!("{}/v1/chat/completions", self.base_url))
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(&body)?;

        // 解析响应
        let json: serde_json::Value = response.into_json()?;
        let message = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("无法从响应中获取文本"))?;

        Ok(message.to_string())
    }
}

impl LLMEngine for Google {
    fn new(options: &OptionMap) -> Self {
        let api_key = option_or_env(&options, "api_key", "GOOGLE_API_KEY");
        let base_url = option_or_env_fallback(
            &options,
            "base_url",
            "GOOGLE_BASE_URL",
            "https://generativelanguage.googleapis.com",
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
            "text": text,
        }));
    }

    fn add_image_content(&mut self, base64_image: &str) {
        self.add_content(json!({
            "inline_data": {
                "mime_type": "image/png",
                "data": base64_image,
            }
        }));
    }

    fn clear_content(&mut self) {
        self.content.clear();
    }
}
