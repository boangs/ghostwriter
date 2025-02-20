use super::LLMEngine;
use crate::util::{option_or_env, option_or_env_fallback, OptionMap};
use anyhow::Result;
use log::debug;
use serde_json::json;
use serde_json::Value as json;
use ureq::Error;

pub struct Tool {
    name: String,
    definition: json,
    callback: Option<Box<dyn FnMut(json)>>,
}

pub struct Anthropic {
    model: String,
    api_key: String,
    base_url: String,
    tools: Vec<Tool>,
    content: Vec<json>,
}

impl Anthropic {
    pub fn add_content(&mut self, content: json) {
        self.content.push(content);
    }

    fn anthropic_tool_definition(tool: &Tool) -> json {
        json!({
            "name": tool.definition["name"],
            "description": tool.definition["description"],
            "input_schema": tool.definition["parameters"],
        })
    }
}

impl LLMEngine for Anthropic {
    fn new(options: &OptionMap) -> Self {
        let api_key = option_or_env(&options, "api_key", "ANTHROPIC_API_KEY");
        let base_url = option_or_env_fallback(
            &options,
            "base_url",
            "ANTHROPIC_BASE_URL",
            "https://api.anthropic.com",
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
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/png",
                "data": base64_image
            }
        }));
    }

    fn clear_content(&mut self) {
        self.content.clear();
    }

    fn execute(&mut self) -> Result<()> {
        let body = json!({
            "model": self.model,
            "max_tokens": 5000,
            "messages": [{
                "role": "user",
                "content": self.content
            }],
            "tools": self.tools.iter().map(|tool| Self::anthropic_tool_definition(tool)).collect::<Vec<_>>(),
            "tool_choice": {
                "type": "any",
                "disable_parallel_tool_use": true
            }
        });

        debug!("Request: {}", body);

        let raw_response = ureq::post(&format!("{}/v1/messages", self.base_url))
            .set("x-api-key", self.api_key.as_str())
            .set("anthropic-version", "2023-06-01")
            .set("Content-Type", "application/json")
            .send_json(&body);

        let response = match raw_response {
            Ok(response) => response,
            Err(Error::Status(code, response)) => {
                debug!("Error: {}", code);
                let json: json = response.into_json()?;
                debug!("Response: {}", json);
                return Err(anyhow::anyhow!("API ERROR"));
            }
            Err(_) => return Err(anyhow::anyhow!("OTHER API ERROR")),
        };

        let json: json = response.into_json().unwrap();
        debug!("Response: {}", json);
        let tool_calls = &json["content"];
        if let Some(tool_call) = tool_calls.get(0) {
            let function_name = tool_call["name"].as_str().unwrap();
            let function_input = &tool_call["input"];
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