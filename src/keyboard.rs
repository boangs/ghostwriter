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
        let start_y: u32 = 100;
        let char_width: u32 = 50;
        let font_size = 30.0;
        
        let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1404" height="1872">"#);
        
        let mut current_x = start_x;
        let mut current_y = start_y;
        
        for c in text.chars() {
            let char_svg = self.font_renderer.char_to_svg(
                c, 
                font_size,
                current_x as i32,
                current_y as i32
            )?;
            svg.push_str(&char_svg);
            debug!("生成字符 {} 的 SVG: {}", c, char_svg);
            
            current_x += char_width;
            if current_x > REMARKABLE_WIDTH - 500 {
                current_y += char_width;
                current_x = start_x;
            }
        }
        
        svg.push_str("</svg>");
        debug!("完整的 SVG: {}", svg);
        
        let bitmap = svg_to_bitmap(&svg, REMARKABLE_WIDTH as u32, REMARKABLE_HEIGHT as u32)?;
        pen.draw_bitmap(&bitmap)?;
        
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
