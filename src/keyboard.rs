use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use crate::constants::{INPUT_WIDTH, INPUT_HEIGHT, REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use crate::font::FontRenderer;
use crate::util::svg_to_bitmap;

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
        
        let start_x: u32 = 100;
        let start_y: u32 = 200;
        let char_width: u32 = 35;
        let font_size = 35.0;
        let paragraph_indent = 70; // 段落缩进（两个字符宽度）
        
        let mut current_x = start_x;
        let mut current_y = start_y;
        
        let mut is_new_paragraph = true;
        
        for line in text.split('\n') {
            if line.trim().is_empty() {
                // 空行表示段落分隔
                current_y += 50; // 段落间增加额外间距
                is_new_paragraph = true;
                continue;
            }
            
            if is_new_paragraph {
                current_x = start_x + paragraph_indent;
                is_new_paragraph = false;
            } else {
                current_x = start_x;
            }
            
            for c in line.chars() {
                self.font_renderer.render_char(c, current_x as f32, current_y as f32, font_size, &mut pen)?;
                current_x += char_width;
                
                // 如果超出页面宽度，换行
                if current_x > REMARKABLE_WIDTH - char_width {
                    current_x = start_x;
                    current_y += 50;
                }
            }
            
            // 每行结束后换行
            current_y += 50;
        }
        
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
