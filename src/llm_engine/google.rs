use super::LLMEngine;
use crate::util::{option_or_env, option_or_env_fallback, OptionMap};
use anyhow::Result;
use log::info;
use serde_json::json;
use serde_json::Value as json;

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
            "type": "image_url",
            "image_url": {
                "url": format!("data:image/png;base64,{}", base64_image)
            }
        }));
    }

    fn clear_content(&mut self) {
        self.content.clear();
    }

    fn execute(&mut self) -> Result<String> {
        info!("执行 Google LLM 引擎");
        
        let body = json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": &self.content
            }]
        });

        let response = ureq::post(&format!("{}/v1/chat/completions", self.base_url))
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(&body)?;

        let json: serde_json::Value = response.into_json()?;
        let message = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("无法从响应中获取文本"))?;

        Ok(message.to_string())
    }
}
