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
        
        // 使用屏幕坐标系 (1404 x 1872)
        let start_x = 200;     // 起始位置
        let start_y = 200;
        let char_width = 100;  // 字符宽度
        let line_height = 150; // 行高
        let scale_factor = 1.0; // 字体缩放
        
        let mut current_x = start_x;
        let mut current_y = start_y;

        for c in text.chars() {
            debug!("开始绘制字符: {} 在位置 ({}, {})", c, current_x, current_y);
            
            // 获取字符的笔画信息并绘制
            if let Ok(strokes) = pen.get_char_strokes(c) {
                for stroke in strokes {
                    if stroke.len() < 2 {
                        continue;
                    }
                    
                    // 移动到笔画起点
                    pen.pen_up()?;
                    let (sx, sy) = stroke[0];
                    let scaled_x = (sx as f32 * scale_factor) as i32 + current_x;
                    let scaled_y = (sy as f32 * scale_factor) as i32 + current_y;
                    debug!("笔画起点: 原始({}, {}) -> 缩放后({}, {})", sx, sy, scaled_x, scaled_y);
                    pen.goto_xy((scaled_x, scaled_y))?;
                    pen.pen_down()?;
                    
                    // 绘制笔画的其余部分
                    for &(px, py) in stroke.iter().skip(1) {
                        let scaled_x = (px as f32 * scale_factor) as i32 + current_x;
                        let scaled_y = (py as f32 * scale_factor) as i32 + current_y;
                        debug!("笔画点: 原始({}, {}) -> 缩放后({}, {})", px, py, scaled_x, scaled_y);
                        pen.goto_xy((scaled_x, scaled_y))?;
                    }
                    
                    pen.pen_up()?;
                    sleep(Duration::from_millis(50));
                }
            }
            
            // 移动到下一个字符位置
            current_x += char_width;
            if current_x > REMARKABLE_WIDTH - 200 {  // 留出右边距
                current_y += line_height;
                current_x = start_x;
                debug!("换行: 从 x={} 到 start_x={}", current_x, start_x);
            }
            
            sleep(Duration::from_millis(100));
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
