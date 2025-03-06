use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread::sleep;
use std::time::Duration;
use crate::constants::REMARKABLE_WIDTH;
use crate::font::FontRenderer;

pub struct Keyboard {
    pen: Arc<Mutex<crate::pen::Pen>>,
    font_renderer: FontRenderer,
    last_y: AtomicU32,
    last_write_top: AtomicU32,    // 记录上次写入的顶部位置
    last_write_bottom: AtomicU32, // 记录上次写入的底部位置
}

impl Keyboard {
    pub fn new(no_draw: bool, _no_draw_progress: bool, initial_y: Option<u32>) -> Result<Self> {
        let initial_y = initial_y.unwrap_or(100);
        Ok(Keyboard {
            pen: Arc::new(Mutex::new(crate::pen::Pen::new(no_draw))),
            font_renderer: FontRenderer::new()?,
            last_y: AtomicU32::new(initial_y),
            last_write_top: AtomicU32::new(initial_y),
            last_write_bottom: AtomicU32::new(initial_y),
        })
    }

    pub fn write_text(&self, text: &str) -> Result<()> {
        debug!("模拟笔书写文本: {}", text);
        let mut pen = self.pen.lock().unwrap();
        
        let start_x: u32 = 100;
        // 始终使用 last_y 作为基准位置
        let start_y = self.last_y.load(Ordering::Relaxed);
        
        let char_width: u32 = 32;
        let line_height: u32 = 38;
        let font_size = 30.0;
        let paragraph_indent = 64; // 段落缩进（两个字符宽度）
        let max_width = REMARKABLE_WIDTH as u32 - 100; // 增加右侧边距
        
        let mut _current_x = start_x;
        let mut current_y = start_y;
        let mut line_start_y = start_y;
        
        let mut is_new_paragraph = true;
        let mut max_y = current_y;  // 跟踪最大的 y 值
        
        for line in text.split('\n') {
            if line.trim().is_empty() {
                // 空行表示段落分隔
                // line_start_y += line_height; // 更新行起始位置
                current_y = line_start_y;
                max_y = max_y.max(current_y);  // 更新最大 y 值
                is_new_paragraph = true;
                continue;
            }
            
            if is_new_paragraph {
                _current_x = start_x + paragraph_indent;
                is_new_paragraph = false;
            } else {
                _current_x = start_x;
            }
            
            // 预先计算这一行是否需要换行
            let mut line_x = _current_x;
            let mut line_chars = Vec::new();
            for c in line.chars() {
                if line_x + char_width > max_width {
                    break;
                }
                line_chars.push(c);
                line_x += char_width;
            }
            
            // 绘制这一行的字符
            for &c in line_chars.iter() {
                // 获取字符的笔画数据和基线偏移
                let (strokes, glyph_baseline) = self.font_renderer.get_char_strokes(c, font_size)?;
                
                // 绘制每个笔画
                for stroke in strokes {
                    if stroke.len() < 2 {
                        continue;
                    }
                    
                    // 移动到笔画起点，使用字形提供的基线偏移
                    let (x, y) = stroke[0];
                    pen.pen_up()?;
                    pen.goto_xy((x + _current_x as i32, y + current_y as i32 + glyph_baseline))?;
                    pen.pen_down()?;
                    
                    // 连续绘制笔画
                    for &(x, y) in stroke.iter().skip(1) {
                        pen.goto_xy((x + _current_x as i32, y + current_y as i32 + glyph_baseline))?;
                        sleep(Duration::from_millis(1));
                    }
                }
                
                _current_x += char_width;
                sleep(Duration::from_millis(2));
            }
            
            // 处理剩余的字符（如果有的话）
            if line_chars.len() < line.chars().count() {
                line_start_y += line_height;
                current_y = line_start_y;
                max_y = max_y.max(current_y);  // 更新最大 y 值
                _current_x = start_x;
                
                for c in line.chars().skip(line_chars.len()) {
                    if _current_x + char_width > max_width {
                        line_start_y += line_height;
                        current_y = line_start_y;
                        max_y = max_y.max(current_y);  // 更新最大 y 值
                        _current_x = start_x;
                    }
                    
                    let (strokes, glyph_baseline) = self.font_renderer.get_char_strokes(c, font_size)?;
                    
                    for stroke in strokes {
                        if stroke.len() < 2 {
                            continue;
                        }
                        
                        let (x, y) = stroke[0];
                        pen.pen_up()?;
                        pen.goto_xy((x + _current_x as i32, y + current_y as i32 + glyph_baseline))?;
                        pen.pen_down()?;
                        
                        for &(x, y) in stroke.iter().skip(1) {
                            pen.goto_xy((x + _current_x as i32, y + current_y as i32 + glyph_baseline))?;
                            sleep(Duration::from_millis(1));
                        }
                    }
                    
                    _current_x += char_width;
                    sleep(Duration::from_millis(10));
                }
            }
            
            // 更新到下一行的起始位置
            line_start_y += line_height;
            current_y = line_start_y;
            max_y = max_y.max(current_y);  // 更新最大 y 值
            _current_x = start_x;
        }
        
        // 不更新 last_y，保持其作为基准位置
        // 只更新 last_write_bottom 用于记录实际写入的位置
        self.last_write_bottom.store(max_y + line_height, Ordering::Relaxed);
        
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

    pub fn write_coordinates(&self) -> Result<()> {
        debug!("开始标记坐标位置");
        let mut pen = self.pen.lock().unwrap();
        
        let start_x: u32 = 800;  // 固定的 x 坐标
        let font_size = 20.0;    // 使用小一点的字体
        
        // 从 20 开始，每隔 20 像素标记一个坐标
        for y in (20..2100).step_by(20) {
            let y_str = y.to_string();
            let mut current_x = start_x;
            
            // 绘制数字
            for c in y_str.chars() {
                let (strokes, glyph_baseline) = self.font_renderer.get_char_strokes(c, font_size)?;
                
                for stroke in strokes {
                    if stroke.len() < 2 {
                        continue;
                    }
                    
                    let (x, y_pos) = stroke[0];
                    pen.pen_up()?;
                    pen.goto_xy((x + current_x as i32, y_pos + y as i32 + glyph_baseline))?;
                    pen.pen_down()?;
                    
                    for &(x, y_pos) in stroke.iter().skip(1) {
                        pen.goto_xy((x + current_x as i32, y_pos + y as i32 + glyph_baseline))?;
                        sleep(Duration::from_millis(1));
                    }
                }
                
                current_x += 16;  // 使用更小的字符间距
                sleep(Duration::from_millis(5));
            }
        }
        
        pen.pen_up()?;
        Ok(())
    }
}
