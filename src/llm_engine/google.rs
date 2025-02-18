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

    fn execute(&mut self) -> Result<String> {
        info!("Executing Google LLM engine");
        
        // 构建请求
        let request = self.build_request()?;
        
        // 发送请求并获取响应
        let response = self.send_request(&request)?;
        
        // 从响应中提取文本
        let text = response.text()?;
        
        Ok(text)
    }
}
