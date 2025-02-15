use super::LLMEngine;
use crate::util::{option_or_env, option_or_env_fallback, OptionMap};
use anyhow::{Result, anyhow};
use serde_json::{json, Value as json};
use ureq;

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
    prompt: String,
    image: Option<String>,
    response: Option<String>,
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
            prompt: String::new(),
            image: None,
            response: None,
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
        let url = format!("{}/v1/chat/completions", self.base_url);
        
        let mut messages = vec![
            json!({
                "role": "system",
                "content": &self.prompt
            })
        ];

        if let Some(image_data) = &self.image {
            messages.push(json!({
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", image_data)
                        }
                    }
                ]
            }));
        }

        let response = ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(json!({
                "model": self.model.clone(),
                "messages": messages,
                "temperature": 0.7,
                "max_tokens": 2000
            }));

        match response {
            Ok(response) => {
                let response_json: json = response.into_json()?;
                if let Some(choices) = response_json.get("choices") {
                    if let Some(first) = choices.get(0) {
                        if let Some(message) = first.get("message") {
                            if let Some(content) = message.get("content") {
                                if let Some(text) = content.as_str() {
                                    self.response = Some(text.to_string());
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
                Err(anyhow!("Invalid API response format: {:?}", response_json))
            }
            Err(err) => {
                if let ureq::Error::Status(code, response) = err {
                    match response.into_json::<json>() {
                        Ok(error_json) => {
                            Err(anyhow!("API Error ({}): {:?}", code, error_json))
                        }
                        Err(_) => {
                            Err(anyhow!("API Error ({}): {}", code, err))
                        }
                    }
                } else {
                    Err(anyhow!("Network Error: {}", err))
                }
            }
        }
    }
}
