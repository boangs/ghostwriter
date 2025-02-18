use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};

pub struct Keyboard {
    pen: Arc<Mutex<crate::pen::Pen>>,
}

impl Keyboard {
    pub fn new(no_draw: bool, no_draw_progress: bool) -> Result<Self> {
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
        let char_width = 50;
        let line_height = 60;
        let mut current_x = start_x;
        let mut current_y = start_y;

        for c in text.chars() {
            // 如果到达行尾，换行
            if current_x > 700 {
                current_x = start_x;
                current_y += line_height;
            }

            // 模拟书写单个字符
            self.write_character(&mut pen, c, current_x, current_y)?;
            
            // 移动到下一个字符位置
            current_x += char_width;
        }
        Ok(())
    }

    fn write_character(&self, pen: &mut crate::pen::Pen, _c: char, x: i32, y: i32) -> Result<()> {
        pen.pen_up()?;
        pen.goto_xy((x, y))?;
        pen.pen_down()?;
        
        pen.goto_xy((x + 40, y))?;
        pen.goto_xy((x + 40, y + 40))?;
        pen.goto_xy((x, y + 40))?;
        pen.goto_xy((x, y))?;
        
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
