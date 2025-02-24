use anyhow::Result;
use log::debug;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
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
                        
                        // 移动到起点
                        let (x, y) = stroke[0];
                        pen.pen_up()?;
                        pen.goto_xy((x + current_x as i32, y + current_y as i32))?;
                        pen.pen_down()?;
                        
                        // 批量创建所有点的事件
                        let mut events = Vec::new();
                        for &(x, y) in stroke.iter().skip(1) {
                            events.push(InputEvent::new(EventType::ABSOLUTE, 0, x + current_x as i32));
                            events.push(InputEvent::new(EventType::ABSOLUTE, 1, y + current_y as i32));
                            events.push(InputEvent::new(EventType::SYNCHRONIZATION, 0, 0));
                        }
                        
                        // 一次性发送所有事件
                        pen.send_events(&events)?;
                    }
                    current_x += char_width;
                }
            }
            
            if current_x > REMARKABLE_WIDTH - 600 {
                current_y += line_height;
                current_x = start_x;
            }
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
