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
use tesseract::Tesseract;
use image::{DynamicImage, ImageBuffer, Luma};
use imageproc::contrast::stretch_contrast;
use imageproc::filter::gaussian_blur_f32;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::json;
use ureq;

pub struct HandwritingInput {
    pen: Arc<Mutex<Pen>>,
    strokes: Vec<Vec<(i32, i32)>>,
    is_writing: bool,
    temp_dir: PathBuf,
    engine: Box<dyn LLMEngine>,
    cache: HashMap<String, (String, u64)>,  // (图像hash, (识别结果, 时间戳))
}

impl HandwritingInput {
    pub fn new(
        no_draw: bool,
        engine: Box<dyn LLMEngine>,
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
            cache: HashMap::new(),
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
        
        // 2. 转换为 base64
        let img_base64 = base64::encode(&img_data);
        
        // 3. 调用百度 OCR API
        let access_token = self.get_baidu_access_token()?;
        let url = format!(
            "https://aip.baidubce.com/rest/2.0/ocr/v1/handwriting?access_token={}",
            access_token
        );
        
        let params = json!({
            "image": img_base64,
            "language_type": "CHN_ENG"
        });
        
        let response = ureq::post(&url)
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_string(&format!("image={}", img_base64))?;
            
        let json: serde_json::Value = response.into_json()?;
        
        // 4. 解析识别结果
        let mut result = String::new();
        if let Some(words_result) = json["words_result"].as_array() {
            for word in words_result {
                if let Some(text) = word["words"].as_str() {
                    result.push_str(text);
                    result.push('\n');
                }
            }
        }
        
        Ok(result)
    }
    
    fn get_baidu_access_token(&self) -> Result<String> {
        let api_key = std::env::var("BAIDU_API_KEY")
            .map_err(|_| anyhow::anyhow!("Missing BAIDU_API_KEY environment variable"))?;
        let secret_key = std::env::var("BAIDU_SECRET_KEY")
            .map_err(|_| anyhow::anyhow!("Missing BAIDU_SECRET_KEY environment variable"))?;
            
        let url = format!(
            "https://aip.baidubce.com/oauth/2.0/token?grant_type=client_credentials&client_id={}&client_secret={}",
            api_key, secret_key
        );
        
        let response = ureq::get(&url).call()?;
        let json: serde_json::Value = response.into_json()?;
        
        if let Some(token) = json["access_token"].as_str() {
            Ok(token.to_string())
        } else {
            Err(anyhow::anyhow!("Failed to get access token"))
        }
    }
} 