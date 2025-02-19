use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};

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

    fn write_character(&self, pen: &mut crate::pen::Pen, c: char, x: i32, y: i32) -> Result<()> {
        // 获取字符的笔画信息
        if let Ok(strokes) = pen.get_char_strokes(c) {
            for stroke in strokes {
                if stroke.len() < 2 {
                    continue;
                }
                
                // 移动到笔画起点
                pen.pen_up()?;
                let (sx, sy) = stroke[0];
                pen.goto_xy((x + sx, y + sy))?;
                pen.pen_down()?;
                
                // 绘制笔画的其余部分
                for &(px, py) in stroke.iter().skip(1) {
                    pen.goto_xy((x + px, y + py))?;
                }
                
                // 笔画之间添加短暂停顿
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            
            // 字符之间添加停顿
            std::thread::sleep(std::time::Duration::from_millis(100));
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
