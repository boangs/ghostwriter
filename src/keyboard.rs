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
        let char_width: u32 = 32;
        let line_height: u32 = 40;
        let font_size = 30.0;
        let paragraph_indent = 64; // 段落缩进（两个字符宽度）
        let baseline_offset = -(font_size as i32); // 增加基线偏移量
        let max_width = REMARKABLE_WIDTH as u32 - 300; // 增加边距
        
        let mut current_x = start_x;
        let mut current_y = start_y;
        let mut line_start_y = start_y;
        
        let mut is_new_paragraph = true;
        
        for line in text.split('\n') {
            if line.trim().is_empty() {
                // 空行表示段落分隔
                line_start_y += line_height; // 更新行起始位置
                current_y = line_start_y;
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
                // 提前检查添加这个字符是否会超出宽度
                if (current_x + char_width) > max_width {
                    line_start_y += line_height;
                    current_y = line_start_y;
                    current_x = start_x;
                }
                
                // 获取字符的笔画数据
                let strokes = self.font_renderer.get_char_strokes(c, font_size)?;
                
                // 绘制每个笔画
                for stroke in strokes {
                    if stroke.len() < 2 {
                        continue;
                    }
                    
                    // 移动到笔画起点，添加基线偏移
                    let (x, y) = stroke[0];
                    pen.pen_up()?;
                    pen.goto_xy((x + current_x as i32, y + current_y as i32 + baseline_offset))?;
                    pen.pen_down()?;
                    
                    // 连续绘制笔画，添加基线偏移
                    for &(x, y) in stroke.iter().skip(1) {
                        pen.goto_xy((x + current_x as i32, y + current_y as i32 + baseline_offset))?;
                        sleep(Duration::from_millis(1));
                    }
                }
                
                current_x += char_width;
                sleep(Duration::from_millis(10));
            }
            
            // 更新到下一行的起始位置
            line_start_y += line_height;
            current_y = line_start_y;
            current_x = start_x;
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
