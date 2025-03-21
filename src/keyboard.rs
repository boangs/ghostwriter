use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread::sleep;
use std::time::Duration;
use crate::constants::REMARKABLE_WIDTH;
use crate::font::{FontRenderer, HersheyFont};

pub struct Keyboard {
    pen: Arc<Mutex<crate::pen::Pen>>,
    font_renderer: FontRenderer,
    hershey_font: HersheyFont,  // 添加 HersheyFont
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
            hershey_font: HersheyFont::new()?,  // 初始化 HersheyFont
            last_y: AtomicU32::new(initial_y),
            last_write_top: AtomicU32::new(initial_y),
            last_write_bottom: AtomicU32::new(initial_y),
        })
    }

    fn is_ascii_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c.is_ascii_punctuation() || c.is_ascii_whitespace()
    }

    pub fn write_text(&self, text: &str) -> Result<()> {
        debug!("模拟笔书写文本: {}", text);
        let mut pen = self.pen.lock().unwrap();
        
        let start_x: f32 = 100.0;
        let start_y = self.last_y.load(Ordering::Relaxed) as f32;
        
        let min_cjk_width: f32 = 65.0;     // 中文字符最小宽度
        let min_ascii_width: f32 = 30.0;    // 英文字符最小宽度
        let line_height: f32 = 65.0;
        let font_size = 60.0;
        let paragraph_indent = 80.0;
        let max_width = REMARKABLE_WIDTH as f32 - 100.0;
        
        let mut current_x = start_x;
        let mut current_y = start_y;
        let mut line_start_y = start_y;
        
        let mut is_new_paragraph = true;
        let mut max_y = current_y;
        
        for line in text.split('\n') {
            if line.trim().is_empty() {
                current_y = line_start_y;
                max_y = max_y.max(current_y);
                is_new_paragraph = true;
                continue;
            }
            
            if is_new_paragraph {
                current_x = start_x + paragraph_indent;
                is_new_paragraph = false;
            } else {
                current_x = start_x;
            }
            
            // 预先计算这一行是否需要换行
            let mut line_x = current_x;
            let mut line_chars = Vec::new();
            for c in line.chars() {
                let (_, _, char_width) = if let Ok(result) = self.hershey_font.get_char_strokes(c, font_size) {
                    result
                } else {
                    self.font_renderer.get_char_strokes(c, font_size)?
                };
                
                // 确保字符宽度不小于最小宽度
                let actual_width = if Self::is_ascii_char(c) {
                    (char_width as f32).max(min_ascii_width)
                } else {
                    (char_width as f32).max(min_cjk_width)
                };
                
                if line_x + actual_width > max_width {
                    break;
                }
                line_chars.push((c, actual_width));
                line_x += actual_width;
            }
            
            // 绘制这一行的字符
            for &(c, char_width) in line_chars.iter() {
                let (strokes, glyph_baseline, _) = if let Ok(result) = self.hershey_font.get_char_strokes(c, font_size) {
                    result
                } else {
                    self.font_renderer.get_char_strokes(c, font_size)?
                };
                
                for stroke in strokes {
                    if stroke.len() < 2 {
                        continue;
                    }
                    
                    let (x, y) = stroke[0];
                    pen.pen_up()?;
                    pen.goto_xy((
                        (x + current_x).round() as i32,
                        (y + current_y + glyph_baseline as f32).round() as i32
                    ))?;
                    pen.pen_down()?;
                    
                    for &(x, y) in stroke.iter().skip(1) {
                        pen.goto_xy((
                            (x + current_x).round() as i32,
                            (y + current_y + glyph_baseline as f32).round() as i32
                        ))?;
                        sleep(Duration::from_millis(5));
                    }
                }
                
                current_x += char_width;
                sleep(Duration::from_millis(10));
            }
            
            // 处理剩余的字符（如果有的话）
            if line_chars.len() < line.chars().count() {
                line_start_y += line_height;
                current_y = line_start_y;
                max_y = max_y.max(current_y);
                current_x = start_x;
                
                for c in line.chars().skip(line_chars.len()) {
                    let (_, _, char_width) = if let Ok(result) = self.hershey_font.get_char_strokes(c, font_size) {
                        result
                    } else {
                        self.font_renderer.get_char_strokes(c, font_size)?
                    };
                    
                    let actual_width = if Self::is_ascii_char(c) {
                        (char_width as f32).max(min_ascii_width)
                    } else {
                        (char_width as f32).max(min_cjk_width)
                    };
                    
                    if current_x + actual_width > max_width {
                        line_start_y += line_height;
                        current_y = line_start_y;
                        max_y = max_y.max(current_y);
                        current_x = start_x;
                    }
                    
                    let (strokes, glyph_baseline, _) = if let Ok(result) = self.hershey_font.get_char_strokes(c, font_size) {
                        result
                    } else {
                        self.font_renderer.get_char_strokes(c, font_size)?
                    };
                    
                    for stroke in strokes {
                        if stroke.len() < 2 {
                            continue;
                        }
                        
                        let (x, y) = stroke[0];
                        pen.pen_up()?;
                        pen.goto_xy((
                            (x + current_x).round() as i32,
                            (y + current_y + glyph_baseline as f32).round() as i32
                        ))?;
                        pen.pen_down()?;
                        
                        for &(x, y) in stroke.iter().skip(1) {
                            pen.goto_xy((
                                (x + current_x).round() as i32,
                                (y + current_y + glyph_baseline as f32).round() as i32
                            ))?;
                            sleep(Duration::from_millis(5));
                        }
                    }
                    
                    current_x += actual_width;
                    sleep(Duration::from_millis(10));
                }
            }
            
            line_start_y += line_height;
            current_y = line_start_y;
            max_y = max_y.max(current_y);
            current_x = start_x;
        }
        
        self.last_write_bottom.store((max_y + line_height) as u32, Ordering::Relaxed);
        
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
        
        let start_x: f32 = 800.0;  // 固定的 x 坐标
        let font_size = 20.0;    // 使用小一点的字体
        
        // 从 20 开始，每隔 20 像素标记一个坐标
        for y in (20..2100).step_by(20) {
            let y_str = y.to_string();
            let mut current_x = start_x;
            
            // 绘制数字
            for c in y_str.chars() {
                let (strokes, glyph_baseline, _) = self.font_renderer.get_char_strokes(c, font_size)?;
                
                for stroke in strokes {
                    if stroke.len() < 2 {
                        continue;
                    }
                    
                    let (x, y_pos) = stroke[0];
                    pen.pen_up()?;
                    pen.goto_xy((
                        (x + current_x).round() as i32,
                        (y_pos + y as f32 + glyph_baseline as f32).round() as i32
                    ))?;
                    pen.pen_down()?;
                    
                    for &(x, y_pos) in stroke.iter().skip(1) {
                        pen.goto_xy((
                            (x + current_x).round() as i32,
                            (y_pos + y as f32 + glyph_baseline as f32).round() as i32
                        ))?;
                        sleep(Duration::from_millis(5));
                    }
                }
                
                current_x += 16.0;  // 使用更小的字符间距
                sleep(Duration::from_millis(5));
            }
        }
        
        pen.pen_up()?;
        Ok(())
    }
}
