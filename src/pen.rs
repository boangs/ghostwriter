use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::thread::sleep;
use std::time::Duration;
use std::fs::OpenOptions;
use std::io::{Write, Seek, SeekFrom};
use std::process::Command;

const REMARKABLE_WIDTH: u32 = 1404;
const REMARKABLE_HEIGHT: u32 = 1872;
const FB_DEVICE: &str = "/dev/fb0";

pub struct Pen {
    no_draw: bool,
    pub framebuffer: Option<std::fs::File>,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        // 在访问 framebuffer 之前先停止 xochitl
        if !no_draw {
            Command::new("systemctl")
                .args(["stop", "xochitl"])
                .output()
                .ok();
        }

        let framebuffer = if !no_draw {
            OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/fb0")
                .ok()
        } else {
            None
        };
        
        Self {
            no_draw,
            framebuffer,
        }
    }

    pub fn cleanup(&mut self) {
        // 在程序结束时重启 xochitl
        if !self.no_draw {
            Command::new("systemctl")
                .args(["start", "xochitl"])
                .output()
                .ok();
        }
    }

    pub fn draw_text(&mut self, text: &str, position: (i32, i32), size: f32) -> Result<()> {
        let font_data = include_bytes!("../assets/WenQuanYiMicroHei.ttf");
        let font = Font::try_from_bytes(font_data).unwrap();
        
        let scale = Scale::uniform(size);
        let v_metrics = font.v_metrics(scale);
        let glyphs: Vec<_> = font.layout(text, scale, Point { 
            x: position.0 as f32, 
            y: position.1 as f32 + v_metrics.ascent 
        }).collect();
        
        for glyph in glyphs {
            if let Some(outline) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    if v > 0.1 {
                        let x = outline.min.x as i32 + x as i32;
                        let y = outline.min.y as i32 + y as i32;
                        self.draw_pixel(x, y);
                    }
                });
            }
        }
        
        Ok(())
    }

    fn draw_pixel(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 || x >= REMARKABLE_WIDTH as i32 || y >= REMARKABLE_HEIGHT as i32 {
            return;
        }

        println!("Draw pixel at ({}, {})", x, y);
        
        if let Some(fb) = &mut self.framebuffer {
            let offset = (y as u32 * REMARKABLE_WIDTH + x as u32) as u64;
            if let Err(e) = fb.seek(SeekFrom::Start(offset)) {
                println!("Failed to seek framebuffer: {}", e);
                return;
            }
            
            if let Err(e) = fb.write_all(&[0xFF]) {
                println!("Failed to write to framebuffer: {}", e);
            }
        }
    }

    pub fn draw_bitmap(&mut self, bitmap: &Vec<Vec<bool>>) -> Result<()> {
        for (y, row) in bitmap.iter().enumerate() {
            for (x, &pixel) in row.iter().enumerate() {
                if pixel {
                    println!("Draw pixel at ({}, {})", x, y);
                }
            }
            sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if let Some(fb) = &mut self.framebuffer {
            fb.flush()?;
        }
        Ok(())
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        self.cleanup();
    }
}
