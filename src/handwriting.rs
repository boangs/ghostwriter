use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::fs;
use std::path::PathBuf;
use base64::prelude::*;
use crate::pen::Pen;
use crate::screenshot::Screenshot;
use crate::llm_engine::{LLMEngine, Message, Role};
use crate::util::OptionMap;
use std::time::Duration;
use std::thread::sleep;
use log::{info, error};
use std::fs::File;
use std::io::Write;

pub struct HandwritingInput {
    pen: Arc<Mutex<Pen>>,
    strokes: Vec<Vec<(i32, i32)>>,
    is_writing: bool,
    temp_dir: PathBuf,
    llm: Box<dyn LLMEngine>,
}

impl HandwritingInput {
    pub fn new(no_draw: bool, llm: Box<dyn LLMEngine>) -> Result<Self> {
        // 创建临时目录
        let temp_dir = std::env::temp_dir().join("ghostwriter");
        fs::create_dir_all(&temp_dir)?;
        
        Ok(Self {
            pen: Arc::new(Mutex::new(Pen::new(no_draw))),
            strokes: Vec::new(),
            is_writing: false,
            temp_dir,
            llm,
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

    pub async fn capture_and_recognize(&self) -> Result<String> {
        // 获取屏幕截图
        let screenshot = Screenshot::new()?;
        let img_data = screenshot.get_image_data()?;

        // 保存截图用于调试
        let mut debug_file = File::create("debug_screenshot.png")?;
        debug_file.write_all(&img_data)?;
        info!("保存截图到 debug_screenshot.png");

        // 将图片转换为 base64
        let base64_image = base64::encode(&img_data);
        info!("图片大小: {} 字节", img_data.len());
        info!("Base64 编码大小: {} 字节", base64_image.len());
        info!("Base64 预览: {}...", &base64_image[..100]);

        // 构建消息
        let system_message = Message::Text {
            role: Role::System,
            content: "你是一个手写文字识别助手。请识别图片中的手写文字内容，直接输出识别结果，不要添加任何额外解释。如果没有看到任何文字，请回复"未检测到文字"。".to_string(),
        };

        let user_message = Message::Image {
            role: Role::User,
            content: "请识别这张图片中的手写文字。".to_string(),
            image_data: base64_image.clone(),
        };

        // 输出请求信息
        info!("API 请求消息:");
        info!("1. System 消息: {}", match &system_message {
            Message::Text { content, .. } => content,
            _ => "",
        });
        info!("2. User 消息: 请识别这张图片中的手写文字。");
        info!("3. 图片数据: data:image/png;base64,{}...", &base64_image[..100]);

        // 发送请求
        let response = self.llm.chat(vec![system_message, user_message]).await?;
        info!("API 响应: {}", response);

        Ok(response)
    }
} 