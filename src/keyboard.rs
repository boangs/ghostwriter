use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

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
        
        // 基础设置
        let start_x = 100;
        let start_y = 100;
        let char_width = 50;  // 每个字符的宽度
        let line_height = 60; // 行高
        let mut current_x = start_x;
        let mut current_y = start_y;

        for c in text.chars() {
            // 如果到达行尾，换行
            if current_x > 700 {
                current_x = start_x;
                current_y += line_height;
            }

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
                    pen.goto_xy((current_x + sx, current_y + sy))?;
                    pen.pen_down()?;
                    
                    // 绘制笔画的其余部分
                    for &(px, py) in stroke.iter().skip(1) {
                        pen.goto_xy((current_x + px, current_y + py))?;
                    }
                    
                    // 笔画之间添加短暂停顿
                    sleep(Duration::from_millis(50));
                }
            }
            
            // 移动到下一个字符位置
            current_x += char_width;
            
            // 字符之间添加停顿
            sleep(Duration::from_millis(100));
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
