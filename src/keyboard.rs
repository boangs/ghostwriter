use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};

pub struct Keyboard {
    pen: Arc<Mutex<crate::pen::Pen>>,
}

impl Keyboard {
    pub fn new() -> Result<Self> {
        Ok(Keyboard {
            pen: Arc::new(Mutex::new(crate::pen::Pen::new(false))),
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
        // 使用 pen 的基础绘制功能来模拟书写
        pen.pen_up()?;
        pen.goto_xy((x, y))?;
        pen.pen_down()?;
        
        // 简单地画一个方框代表一个字
        // 使用 goto_xy 和 pen up/down 来画线
        pen.goto_xy((x + 40, y))?;
        pen.goto_xy((x + 40, y + 40))?;
        pen.goto_xy((x, y + 40))?;
        pen.goto_xy((x, y))?;
        
        pen.pen_up()?;
        Ok(())
    }

    pub fn write_progress(&self, _progress: f32) -> Result<()> {
        // 可以选择是否显示进度，这里简化处理
        Ok(())
    }
}
