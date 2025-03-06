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
        let max_width = REMARKABLE_WIDTH as u32 - 500; // 增加右侧边距
        
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
                sleep(Duration::from_millis(10));
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

    pub fn draw_coordinate_system(&self) -> Result<()> {
        let mut pen = self.pen.lock().unwrap();
        let font_size = 20.0;
        let step = 200;  // 每隔200像素画一个刻度，避免太密集
        
        // 画设备边界
        pen.pen_up()?;
        pen.goto_xy((0, 0))?;  // 左上角
        pen.pen_down()?;
        pen.goto_xy((1620, 0))?;  // 右上角
        pen.goto_xy((1620, 2160))?;  // 右下角
        pen.goto_xy((0, 2160))?;  // 左下角
        pen.goto_xy((0, 0))?;  // 回到左上角
        pen.pen_up()?;
        
        // 画十字线
        // 横线（设备中心水平线）
        pen.pen_up()?;
        pen.goto_xy((0, 1080))?;  // 从左边开始
        pen.pen_down()?;
        pen.goto_xy((1620, 1080))?;  // 画到右边
        
        // 竖线（设备中心垂直线）
        pen.pen_up()?;
        pen.goto_xy((810, 0))?;  // 从上面开始
        pen.pen_down()?;
        pen.goto_xy((810, 2160))?;  // 画到下面
        
        // 在横线上标记刻度和坐标
        for x in (0..=1620).step_by(step) {
            pen.pen_up()?;
            pen.goto_xy((x, 1070))?;  // 刻度线起点
            pen.pen_down()?;
            pen.goto_xy((x, 1090))?;  // 刻度线终点
            
            // 写坐标值
            pen.pen_up()?;
            pen.goto_xy((x, 1100))?;
            self.write_text(&format!("{}", x))?;
        }
        
        // 在竖线上标记刻度和坐标
        for y in (0..=2160).step_by(step) {
            pen.pen_up()?;
            pen.goto_xy((800, y))?;  // 刻度线起点
            pen.pen_down()?;
            pen.goto_xy((820, y))?;  // 刻度线终点
            
            // 写坐标值
            pen.pen_up()?;
            pen.goto_xy((830, y))?;
            self.write_text(&format!("{}", y))?;
        }
        
        // 在中心点写上坐标
        pen.pen_up()?;
        pen.goto_xy((820, 1100))?;
        self.write_text("(810,1080)")?;
        
        // 标注设备尺寸
        pen.pen_up()?;
        pen.goto_xy((10, 30))?;
        self.write_text("设备实际尺寸: 1620x2160")?;
        
        pen.pen_up()?;
        Ok(())
    }
}
