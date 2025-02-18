use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};

pub struct Keyboard {
    pen: Arc<Mutex<crate::pen::Pen>>,
}

impl Keyboard {
    pub fn new() -> Result<Self> {
        Ok(Keyboard {
            pen: Arc::new(Mutex::new(crate::pen::Pen::new(false)?)),
        })
    }

    pub fn write_text(&self, text: &str) -> Result<()> {
        debug!("模拟笔书写文本: {}", text);
        let mut pen = self.pen.lock().unwrap();
        
        // 基础设置
        let start_x = 100.0;
        let start_y = 100.0;
        let char_width = 50.0;
        let line_height = 60.0;
        let mut current_x = start_x;
        let mut current_y = start_y;

        for c in text.chars() {
            // 如果到达行尾，换行
            if current_x > 700.0 {
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

    fn write_character(&self, pen: &mut crate::pen::Pen, c: char, x: f32, y: f32) -> Result<()> {
        // 这里需要实现具体的笔画绘制逻辑
        // 可以使用预定义的笔画数据库或者简化的笔画系统
        
        // 示例：简单地画一个方框代表一个字
        pen.begin_draw()?;
        
        // 画一个方框表示字符边界
        pen.move_to(x, y)?;
        pen.line_to(x + 40.0, y)?;
        pen.line_to(x + 40.0, y + 40.0)?;
        pen.line_to(x, y + 40.0)?;
        pen.line_to(x, y)?;
        
        pen.end_draw()?;
        Ok(())
    }

    pub fn write_progress(&self, _progress: f32) -> Result<()> {
        // 可以选择是否显示进度，这里简化处理
        Ok(())
    }
}
