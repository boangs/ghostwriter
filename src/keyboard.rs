use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use crate::constants::{INPUT_WIDTH, INPUT_HEIGHT, REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use crate::font::FontRenderer;

pub struct Keyboard {
    pen: Arc<Mutex<crate::pen::Pen>>,
    font_renderer: FontRenderer,
}

impl Keyboard {
    pub fn new(no_draw: bool, _no_draw_progress: bool) -> Result<Self> {
        Ok(Keyboard {
            pen: Arc::new(Mutex::new(crate::pen::Pen::new(no_draw))),
            font_renderer: FontRenderer::new()?,
        })
    }

    pub fn write_text(&self, text: &str) -> Result<()> {
        debug!("模拟笔书写文本: {}", text);
        let mut pen = self.pen.lock().unwrap();
        
        // 调整起始位置和字符大小
        let start_x: u32 = 100;      // 左边距
        let start_y: u32 = 200;      // 上边距
        let char_width: u32 = 80;    // 字符宽度
        let line_height: u32 = 100;  // 行高
        let font_size = 50.0;        // 字体大小
        
        let mut current_x = start_x;
        let mut current_y = start_y;

        for c in text.chars() {
            debug!("开始绘制字符: {} 在位置 ({}, {})", c, current_x, current_y);
            
            // 获取字符的位图点
            let points = self.font_renderer.get_char_bitmap(c, font_size);
            
            // 绘制每个点
            for (x, y) in points {
                let screen_x = x as i32 + current_x as i32;
                let screen_y = y as i32 + current_y as i32;
                
                pen.pen_down()?;
                pen.goto_xy((screen_x, screen_y))?;
                pen.pen_up()?;
            }
            
            current_x += char_width;
            if current_x > REMARKABLE_WIDTH - 100 {
                current_y += line_height;
                current_x = start_x;
                
                if current_y > REMARKABLE_HEIGHT - 100 {
                    current_y = start_y;
                }
            }
            
            sleep(Duration::from_millis(50));
        }
        
        pen.pen_up()?;
        Ok(())
    }

    pub fn progress(&self) -> Result<()> {
        Ok(())
    }

    pub fn progress_end(&self) -> Result<()> {
        Ok(())
    }

    pub fn key_cmd_body(&self) -> Result<()> {
        Ok(())
    }

    pub fn string_to_keypresses(&self, _text: &str) -> Result<()> {
        Ok(())
    }

    pub fn write_progress(&self, _progress: f32) -> Result<()> {
        Ok(())
    }
}
