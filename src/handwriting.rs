use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::fs;
use std::path::PathBuf;
use base64::{Engine, engine::general_purpose::STANDARD};
use crate::pen::Pen;
use crate::screenshot::Screenshot;
use crate::llm_engine::LLMEngine;
use crate::font::{FontRenderer, HersheyFont};
use std::time::Duration;
use std::thread::sleep;
use log::{info, error, debug};
use serde_json::json;
use ureq;
use crate::constants::REMARKABLE_WIDTH;
use crate::constants::REMARKABLE_HEIGHT;

pub struct HandwritingInput {
    pen: Arc<Mutex<Pen>>,
    strokes: Vec<Vec<(i32, i32)>>,
    is_writing: bool,
    temp_dir: PathBuf,
    engine: Box<dyn LLMEngine>,
    font_renderer: FontRenderer,
    hershey_font: HersheyFont,
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
            font_renderer: FontRenderer::new()?,
            hershey_font: HersheyFont::new()?,
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
                sleep(Duration::from_millis(2));
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

    pub fn capture_and_recognize(&mut self) -> Result<(String, i32)> {
        info!("开始截图和识别过程");
        
        // 1. 截取当前屏幕
        let mut screenshot = Screenshot::new()?;
        let img_data = screenshot.get_image_data()?;
        
        // 获取最后一行内容的 y 坐标
        let last_y = screenshot.find_last_content_y();
        info!("找到最后一行内容的 y 坐标: {}", last_y);
        
        // 仅为调试目的保存图片
        if cfg!(debug_assertions) {
            let debug_image_path = self.temp_dir.join("debug_screenshot.png");
            if let Err(e) = std::fs::write(&debug_image_path, &img_data) {
                error!("保存调试截图失败: {}", e);
            } else {
                info!("保存调试截图到: {}", debug_image_path.display());
            }
        }
        
        // 2. 直接使用内存中的图片数据转换为 base64
        let img_base64 = STANDARD.encode(&img_data);
        info!("图片已转换为 base64，长度: {} 字符", img_base64.len());
        
        // 3. 调用百度 OCR API
        info!("开始调用百度 OCR API");
        let access_token = self.get_baidu_access_token()?;
        let url = format!(
            "https://aip.baidubce.com/rest/2.0/ocr/v1/handwriting?access_token={}",
            access_token
        );
        
        // 构建请求参数
        let params = [
            ("image", img_base64.as_str()),
            ("language_type", "CHN_ENG"),
            ("detect_direction", "true"),
            ("probability", "true"),
        ];
        
        let response = ureq::post(&url)
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_form(&params)?;
            
        info!("成功获取 API 响应");
            
        let json: serde_json::Value = response.into_json()?;
        
        // 仅为调试目的保存 API 响应
        if cfg!(debug_assertions) {
            let debug_response_path = self.temp_dir.join("debug_response.json");
            if let Err(e) = std::fs::write(&debug_response_path, serde_json::to_string_pretty(&json)?) {
                error!("保存 API 响应失败: {}", e);
            } else {
                info!("保存 API 响应到: {}", debug_response_path.display());
            }
        }
        
        // 检查是否有错误
        if let Some(_error_code) = json.get("error_code") {
            error!("百度 OCR API 返回错误: {:?}", json);
            return Err(anyhow::anyhow!("百度 OCR API 错误: {}", json["error_msg"].as_str().unwrap_or("未知错误")));
        }
        
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
        debug!("识别结果: {}", result);

        // 仅为调试目的保存识别结果
        if cfg!(debug_assertions) {
            let debug_text_path = self.temp_dir.join("debug_result.txt");
            if let Err(e) = std::fs::write(&debug_text_path, &result) {
                error!("保存识别结果失败: {}", e);
            } else {
                info!("保存识别结果到: {}", debug_text_path.display());
            }
        }

        // 5. 将识别结果传给 AI 引擎
        info!("开始处理 AI 引擎响应");
        self.engine.clear_content();
        self.engine.add_text_content(&format!(
            "{}\n",
            result.trim()
        ));
        
        let response = Arc::new(Mutex::new(String::new()));
        let response_clone = response.clone();
        
        self.engine.register_tool(
            "write",
            json!({
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
        
        // 7. 执行识别
        info!("执行 AI 引擎");
        self.engine.execute()?;
        
        // 8. 返回识别结果（而不是 AI 的回复）和位置
        info!("完成识别过程");
        Ok((result, last_y))
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

    pub fn write_text(&mut self, text: &str, x: i32, y: i32) -> Result<()> {
        let mut pen = self.pen.lock().unwrap();
        let font_size = 18.0;
        
        // 基础间距设置
        let base_spacing_ratio = 0.2; // 基础间距为字符宽度的 20%
        let min_spacing = font_size * 0.1; // 最小间距为字体大小的 10%
        let line_height = font_size * 3.0; // 行高为字体大小的 1.5 倍
        let bottom_margin = 100.0; // 底部留白
        
        let mut current_x = x as f32;
        let mut current_y = y as f32;
        
        for c in text.chars() {
            
            if c == '\n' {
                // 处理换行
                current_x = x as f32;
                current_y += line_height;
                // 检查是否需要换页
                if current_y > REMARKABLE_HEIGHT as f32 - bottom_margin {
                    current_y = y as f32; // 回到顶部
                }
                continue;
            }
            
            // 尝试使用 Hershey 字体，如果失败则回退到 FreeType
            let (strokes, baseline_offset, char_width) = match self.hershey_font.get_char_strokes(c, font_size) {
                Ok(result) => result,
                Err(_) => self.font_renderer.get_char_strokes(c, font_size)?
            };
            
            // 检查是否需要换页
            if current_y > REMARKABLE_HEIGHT as f32 - bottom_margin {
                current_y = y as f32; // 回到顶部
                current_x = x as f32;
            }
            
            // 绘制笔画
            for stroke in strokes {
                if stroke.len() < 2 {
                    continue;
                }
                
                let (start_x, start_y) = stroke[0];
                pen.pen_up()?;
                // 在最后一步转换为整数
                pen.goto_xy((
                    (start_x + current_x).round() as i32,
                    (start_y + current_y + baseline_offset as f32).round() as i32
                ))?;
                pen.pen_down()?;
                
                for &(x, y) in stroke.iter().skip(1) {
                    // 在每个点之间也检查橡皮擦，提高响应速度
                    if pen.check_real_eraser()? {
                        info!("检测到橡皮擦接触，终止绘制过程");
                        pen.pen_up()?;
                        return Ok(());  // 直接返回，结束整个绘制过程
                    }
                    
                    pen.goto_xy((
                        (x + current_x).round() as i32,
                        (y + current_y + baseline_offset as f32).round() as i32
                    ))?;
                }
            }
            
            // 计算字符间距
            let char_width = char_width as f32;
            let spacing = if c.is_ascii() {
                // 英文字符使用更小的间距，并考虑字符宽度
                (char_width * base_spacing_ratio * 0.8).max(min_spacing)
            } else {
                // 中文字符使用标准间距
                (char_width * base_spacing_ratio).max(min_spacing)
            };
            
            // 增加字符宽度和额外的间距
            current_x += char_width + spacing;
            
            // 如果超出屏幕宽度，换行
            if current_x > REMARKABLE_WIDTH as f32 - 100.0 {
                current_x = x as f32;
                current_y += line_height;
                // 检查是否需要换页
                if current_y > REMARKABLE_HEIGHT as f32 - bottom_margin {
                    current_y = y as f32; // 回到顶部
                }
                sleep(Duration::from_millis(10));
            }
            
             sleep(Duration::from_millis(10));
        }
        
        pen.pen_up()?;
        Ok(())
    }
}