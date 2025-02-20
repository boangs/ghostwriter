use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use crate::constants::{INPUT_WIDTH, INPUT_HEIGHT, REMARKABLE_WIDTH, REMARKABLE_HEIGHT};

pub struct Keyboard {
    pen: Arc<Mutex<crate::pen::Pen>>,
}

impl Keyboard {
    pub fn new(no_draw: bool, _no_draw_progress: bool) -> Result<Self> {
        Ok(Keyboard {
            pen: Arc::new(Mutex::new(crate::pen::Pen::new(no_draw))),
        })
    }

    pub fn write_text(&self, text: &str) -> Result<()> {
        debug!("模拟笔书写文本: {}", text);
        let mut pen = self.pen.lock().unwrap();
        
        // 调整起始位置和字符大小
        let start_x: u32 = 100;      // 左边距
        let start_y: u32 = 200;      // 上边距
        let char_width: u32 = 30;    // 字符宽度
        let line_height: u32 = 20;  // 行高
        let scale_factor = 0.02;      // 缩小字体缩放因子
        
        let mut current_x = start_x;
        let mut current_y = start_y;

        for c in text.chars() {
            debug!("开始绘制字符: {} 在位置 ({}, {})", c, current_x, current_y);
            
            if let Ok(strokes) = pen.get_char_strokes(c) {
                for stroke in strokes {
                    if stroke.len() < 2 {
                        continue;
                    }
                    
                    pen.pen_up()?;
                    let (sx, sy) = stroke[0];
                    // 翻转 y 坐标并应用缩放
                    let scaled_x = (sx as f32 * scale_factor) as i32 + current_x as i32;
                    let scaled_y = (-sy as f32 * scale_factor) as i32 + current_y as i32;
                    debug!("笔画起点: 原始({}, {}) -> 缩放后({}, {})", sx, sy, scaled_x, scaled_y);
                    pen.goto_xy((scaled_x, scaled_y))?;
                    pen.pen_down()?;
                    
                    for &(px, py) in stroke.iter().skip(1) {
                        let scaled_x = (px as f32 * scale_factor) as i32 + current_x as i32;
                        let scaled_y = (-py as f32 * scale_factor) as i32 + current_y as i32;
                        debug!("笔画点: 原始({}, {}) -> 缩放后({}, {})", px, py, scaled_x, scaled_y);
                        pen.goto_xy((scaled_x, scaled_y))?;
                    }
                    
                    pen.pen_up()?;
                    sleep(Duration::from_millis(50));
                }
            }
            
            current_x += char_width;
            if current_x > REMARKABLE_WIDTH - 500 {
                current_y += line_height;
                current_x = start_x;
                
                if current_y > REMARKABLE_HEIGHT - 500 {
                    current_y = start_y;
                }
            }
            
            sleep(Duration::from_millis(10));
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
