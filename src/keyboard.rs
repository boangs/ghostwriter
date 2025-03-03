use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use crate::constants::REMARKABLE_WIDTH;
use crate::font::FontRenderer;
use image::{GrayImage, GenericImageView};
use crate::screenshot::Screenshot;

pub struct Keyboard {
    pen: Arc<Mutex<crate::pen::Pen>>,
    font_renderer: FontRenderer,
    last_y: u32,  // 添加一个字段来记录最后写入的位置
}

impl Keyboard {
    pub fn new(no_draw: bool, _no_draw_progress: bool) -> Result<Self> {
        Ok(Keyboard {
            pen: Arc::new(Mutex::new(crate::pen::Pen::new(no_draw))),
            font_renderer: FontRenderer::new()?,
            last_y: 100,  // 初始位置
        })
    }

    // 检测屏幕上已有内容的最后一行位置
    fn detect_last_content_line() -> Result<u32> {
        let screenshot = Screenshot::new()?;
        let img_data = screenshot.get_image_data()?;
        let img = image::load_from_memory(&img_data)?;
        let gray_img = img.to_luma8();
        
        let height = gray_img.height();
        let width = gray_img.width();
        let mut last_content_line = 100; // 默认从顶部开始
        
        // 从下往上扫描，找到第一行有内容的位置
        for y in (100..height-100).rev() {
            let mut has_content = false;
            for x in 100..width-100 {
                if gray_img.get_pixel(x, y)[0] < 200 { // 检测非白色像素
                    has_content = true;
                    break;
                }
            }
            if has_content {
                last_content_line = y + 60; // 在最后一行内容下方留出一些空间
                break;
            }
        }
        
        Ok(last_content_line)
    }

    pub fn write_text(&self, text: &str) -> Result<()> {
        debug!("模拟笔书写文本: {}", text);
        let mut pen = self.pen.lock().unwrap();
        
        let start_x: u32 = 100;
        let start_y = self.last_y;  // 使用记录的最后位置
        let char_width: u32 = 32;
        let line_height: u32 = 40;
        let font_size = 30.0;
        let paragraph_indent = 64; // 段落缩进（两个字符宽度）
        let max_width = REMARKABLE_WIDTH as u32 - 500; // 增加右侧边距
        
        let mut current_x = start_x;
        let mut current_y = start_y;
        let mut line_start_y = start_y;
        
        let mut is_new_paragraph = true;
        let mut max_y = current_y;  // 跟踪最大的 y 值
        
        for line in text.split('\n') {
            if line.trim().is_empty() {
                // 空行表示段落分隔
                line_start_y += line_height; // 更新行起始位置
                current_y = line_start_y;
                max_y = max_y.max(current_y);  // 更新最大 y 值
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
                    pen.goto_xy((x + current_x as i32, y + current_y as i32 + glyph_baseline))?;
                    pen.pen_down()?;
                    
                    // 连续绘制笔画
                    for &(x, y) in stroke.iter().skip(1) {
                        pen.goto_xy((x + current_x as i32, y + current_y as i32 + glyph_baseline))?;
                        sleep(Duration::from_millis(1));
                    }
                }
                
                current_x += char_width;
                sleep(Duration::from_millis(10));
            }
            
            // 处理剩余的字符（如果有的话）
            if line_chars.len() < line.chars().count() {
                line_start_y += line_height;
                current_y = line_start_y;
                max_y = max_y.max(current_y);  // 更新最大 y 值
                current_x = start_x;
                
                for c in line.chars().skip(line_chars.len()) {
                    if current_x + char_width > max_width {
                        line_start_y += line_height;
                        current_y = line_start_y;
                        max_y = max_y.max(current_y);  // 更新最大 y 值
                        current_x = start_x;
                    }
                    
                    let (strokes, glyph_baseline) = self.font_renderer.get_char_strokes(c, font_size)?;
                    
                    for stroke in strokes {
                        if stroke.len() < 2 {
                            continue;
                        }
                        
                        let (x, y) = stroke[0];
                        pen.pen_up()?;
                        pen.goto_xy((x + current_x as i32, y + current_y as i32 + glyph_baseline))?;
                        pen.pen_down()?;
                        
                        for &(x, y) in stroke.iter().skip(1) {
                            pen.goto_xy((x + current_x as i32, y + current_y as i32 + glyph_baseline))?;
                            sleep(Duration::from_millis(1));
                        }
                    }
                    
                    current_x += char_width;
                    sleep(Duration::from_millis(10));
                }
            }
            
            // 更新到下一行的起始位置
            line_start_y += line_height;
            current_y = line_start_y;
            max_y = max_y.max(current_y);  // 更新最大 y 值
            current_x = start_x;
        }
        
        // 更新最后写入的位置
        unsafe {
            let self_mut = &mut *(self as *const _ as *mut Self);
            self_mut.last_y = max_y + line_height;  // 为下次写入预留一行的空间
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
