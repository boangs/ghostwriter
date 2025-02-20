pub mod anthropic;
pub mod google;
pub mod openai;

use anyhow::Result;
use serde_json::Value as json;
use std::collections::HashMap;

pub trait LLMEngine {
    fn new(options: &HashMap<String, String>) -> Self
    where
        Self: Sized;
    fn register_tool(&mut self, name: &str, definition: json, callback: Box<dyn FnMut(json)>);
    fn add_text_content(&mut self, text: &str);
    fn add_image_content(&mut self, base64_image: &str);
    fn clear_content(&mut self);
    fn execute(&mut self) -> Result<()>;
}