use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use std::fs::File;
use std::io::Write;
use crate::constants::{INPUT_WIDTH, INPUT_HEIGHT, REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use crate::font::FontRenderer;
use crate::util::svg_to_bitmap;
use evdev::{Device, EventType, InputEvent};

pub struct Keyboard {
    pen: Arc<Mutex<crate::pen::Pen>>,
    font_renderer: FontRenderer,
}

impl Keyboard {
    pub fn new(no_draw: bool, _no_draw_progress: bool) -> Result<Self> {
        Ok(Keyboard {
            pen: Arc::new(Mutex::new(crate::pen::Pen::new(no_draw))),
            font_renderer: FontRenderer::new()?,
        })
    }

    pub fn write_text(&self, text: &str) -> Result<()> {
        debug!("模拟笔书写文本: {}", text);
        let mut pen = self.pen.lock().unwrap();
        
        let start_x: u32 = 100;
        let start_y: u32 = 100;
        let char_width: u32 = 35;
        let line_height: u32 = 40;
        let font_size = 32.0;
        
        let mut current_x = start_x;
        let mut current_y = start_y;
        
        for c in text.chars() {
            match c {
                ' ' => {
                    current_x += char_width;
                }
                '\n' => {
                    current_y += line_height;
                    current_x = start_x;
                }
                _ => {
                    let strokes = self.font_renderer.get_char_strokes(c, font_size)?;
                    for stroke in strokes {
                        if stroke.len() < 2 {
                            continue;
                        }
                        
                        // 移动到笔画起点
                        let (x, y) = stroke[0];
                        pen.pen_up()?;
                        pen.goto_xy((x + current_x as i32, y + current_y as i32))?;
                        pen.pen_down()?;
                        
                        // 连续绘制笔画
                        for &(x, y) in stroke.iter().skip(1) {
                            pen.goto_xy((x + current_x as i32, y + current_y as i32))?;
                            sleep(Duration::from_millis(1));
                        }
                    }
                    current_x += char_width;
                }
            }
            
            if current_x > REMARKABLE_WIDTH - 600 {
                current_y += line_height;
                current_x = start_x;
            }
            
            sleep(Duration::from_millis(5));
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

    // 添加新方法，直接打印文本到屏幕
    pub fn print_text_to_screen(&self, text: &str) -> Result<()> {
        debug!("直接打印文本到屏幕: {}", text);
        
        let mut svg = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1620" height="2160">
"#);
        
        let start_x = 100;
        let start_y = 200;
        let line_height = 40;
        let font_size = 32.0;
        
        let mut current_x = start_x;
        let mut current_y = start_y;
        
        for c in text.chars() {
            match c {
                '\n' => {
                    current_y += line_height;
                    current_x = start_x;
                }
                ' ' => {
                    current_x += 20;
                }
                _ => {
                    // 使用 char_to_svg 方法获取字符的 SVG 路径
                    let char_svg = self.font_renderer.char_to_svg(c, font_size, current_x, current_y)?;
                    svg.push_str(&char_svg);
                    current_x += 35;
                    
                    if current_x > REMARKABLE_WIDTH - 600 {
                        current_y += line_height;
                        current_x = start_x;
                    }
                }
            }
        }
        
        svg.push_str("</svg>");
        
        // 将 SVG 转换为位图并绘制
        let bitmap = svg_to_bitmap(&svg, REMARKABLE_WIDTH, REMARKABLE_HEIGHT)?;
        let mut pen = self.pen.lock().unwrap();
        pen.draw_bitmap(&bitmap)?;
        
        Ok(())
    }
    
    // 直接写入到帧缓冲区
    fn write_to_framebuffer(&self, svg: &str) -> Result<()> {
        // 保存 SVG 到临时文件
        let temp_svg_path = "/tmp/remarkable_text.svg";
        let mut file = File::create(temp_svg_path)?;
        file.write_all(svg.as_bytes())?;
        
        // 使用 reMarkable 的工具将 SVG 渲染到屏幕
        // 注意：这需要 root 权限
        std::process::Command::new("epframebuffer")
            .args(&["draw", temp_svg_path])
            .status()?;
        
        Ok(())
    }
}
