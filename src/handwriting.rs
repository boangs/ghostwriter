use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::fs;
use std::path::PathBuf;
use base64::prelude::*;
use crate::pen::Pen;
use crate::screenshot::Screenshot;
use crate::llm_engine::LLMEngine;
use crate::util::OptionMap;
use std::time::Duration;
use std::thread::sleep;

pub struct HandwritingInput {
    pen: Arc<Mutex<Pen>>,
    strokes: Vec<Vec<(i32, i32)>>,
    is_writing: bool,
    temp_dir: PathBuf,
    engine: Box<dyn LLMEngine>,
}

impl HandwritingInput {
    pub fn new(no_draw: bool, engine: Box<dyn LLMEngine>) -> Result<Self> {
        // 创建临时目录
        let temp_dir = std::env::temp_dir().join("ghostwriter");
        fs::create_dir_all(&temp_dir)?;
        
        Ok(Self {
            pen: Arc::new(Mutex::new(Pen::new(no_draw))),
            strokes: Vec::new(),
            is_writing: false,
            temp_dir,
            engine,
        })
    }

    pub fn start_stroke(&mut self, x: i32, y: i32) -> Result<()> {
        self.is_writing = true;
        let mut current_stroke = Vec::new();
        current_stroke.push((x, y));
        self.strokes.push(current_stroke);
        
        let mut pen = self.pen.lock().unwrap();
        pen.pen_down()?;
        pen.goto_xy((x, y))?;
        Ok(())
    }

    pub fn continue_stroke(&mut self, x: i32, y: i32) -> Result<()> {
        if self.is_writing {
            if let Some(current_stroke) = self.strokes.last_mut() {
                current_stroke.push((x, y));
                let mut pen = self.pen.lock().unwrap();
                pen.goto_xy((x, y))?;
                sleep(Duration::from_millis(1));
            }
        }
        Ok(())
    }

    pub fn end_stroke(&mut self) -> Result<()> {
        self.is_writing = false;
        let mut pen = self.pen.lock().unwrap();
        pen.pen_up()?;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.strokes.clear();
        self.is_writing = false;
    }

    pub fn capture_and_recognize(&mut self) -> Result<String> {
        // 1. 截取当前屏幕
        let screenshot = Screenshot::new()?;
        let img_data = screenshot.get_image_data()?;
        
        // 2. 将图像转换为 base64
        let base64_image = base64::encode(&img_data);
        
        // 3. 清除之前的内容
        self.engine.clear_content();
        
        // 4. 添加提示词和图片
        self.engine.add_text_content("请识别图片中的手写文字内容。直接输出识别到的文字，不要解释。");
        self.engine.add_image_content(&base64_image);
        
        // 5. 注册回调处理识别结果
        let response = Arc::new(Mutex::new(String::new()));
        let response_clone = response.clone();
        
        self.engine.register_tool(
            "write",
            serde_json::json!({
                "name": "write",
                "description": "Write the recognized text",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The recognized text"
                        }
                    },
                    "required": ["text"]
                }
            }),
            Box::new(move |args: serde_json::Value| {
                let text = args["text"].as_str().unwrap_or_default();
                *response_clone.lock().unwrap() = text.to_string();
            })
        );
        
        // 6. 执行识别
        self.engine.execute()?;
        
        // 7. 返回识别结果
        Ok(response.lock().unwrap().clone())
    }
} 