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
use log;
use sha2::{Sha256, Digest};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct HandwritingInput {
    pen: Arc<Mutex<Pen>>,
    strokes: Vec<Vec<(i32, i32)>>,
    is_writing: bool,
    temp_dir: PathBuf,
    engine: Box<dyn LLMEngine>,
    app_key: String,
    app_secret: String,
}

impl HandwritingInput {
    pub fn new(
        no_draw: bool,
        engine: Box<dyn LLMEngine>,
        app_key: String,
        app_secret: String,
    ) -> Result<Self> {
        // 创建临时目录
        let temp_dir = std::env::temp_dir().join("ghostwriter");
        fs::create_dir_all(&temp_dir)?;
        
        Ok(Self {
            pen: Arc::new(Mutex::new(Pen::new(no_draw))),
            strokes: Vec::new(),
            is_writing: false,
            temp_dir,
            engine,
            app_key,
            app_secret,
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
        
        // 3. 准备有道 API 参数
        let salt = uuid::Uuid::new_v4().to_string();
        let curtime = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs()
            .to_string();
            
        // 计算 input
        let img_len = base64_image.len().to_string();
        let input = if base64_image.len() > 20 {
            format!(
                "{}{}{}",
                &base64_image[..10],
                img_len,
                &base64_image[base64_image.len()-10..]
            )
        } else {
            base64_image.clone()
        };
        
        // 计算签名
        let sign_str = format!(
            "{}{}{}{}{}",
            self.app_key, input, curtime, salt, self.app_secret
        );
        let mut hasher = Sha256::new();
        hasher.update(sign_str.as_bytes());
        let sign = format!("{:x}", hasher.finalize());
        
        // 4. 发送请求
        let res = ureq::post("https://openapi.youdao.com/ocr_hand_writing")
            .send_form(&[
                ("appKey", self.app_key.as_str()),
                ("salt", salt.as_str()),
                ("curtime", curtime.as_str()),
                ("sign", sign.as_str()),
                ("signType", "v3"),
                ("langType", "zh-CHS"),
                ("imageType", "1"),
                ("img", base64_image.as_str()),
            ])?;
            
        let json: Value = res.into_json()?;
        
        // 5. 解析结果
        if json["errorCode"].as_str() == Some("0") {
            // 提取识别文本
            let mut result = String::new();
            if let Some(regions) = json["Result"]["regions"].as_array() {
                for region in regions {
                    if let Some(lines) = region["lines"].as_array() {
                        for line in lines {
                            if let Some(text) = line["text"].as_str() {
                                result.push_str(text);
                                result.push('\n');
                            }
                        }
                    }
                }
            }
            
            // 6. 将识别结果传给 AI 引擎
            self.engine.clear_content();
            self.engine.add_text_content(&format!(
                "识别到的手写文字内容是:\n{}\n请对这段文字进行分析和回应。",
                result.trim()
            ));
            
            // 7. 注册回调处理识别结果
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
            
            // 8. 执行识别
            self.engine.execute()?;
            
            // 9. 返回识别结果
            let result = response.lock().unwrap().clone();
            Ok(result)
            
        } else {
            Err(anyhow::anyhow!(
                "识别失败: {}",
                json["errorCode"].as_str().unwrap_or("未知错误")
            ))
        }
    }
} 