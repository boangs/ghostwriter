use anyhow::Result;
use rusttype::{Font, Scale, Point};
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
        // 不再停止 xochitl
        let framebuffer = if !no_draw {
            // 尝试以只写方式打开帧缓冲区
            OpenOptions::new()
                .write(true)
                .open(FB_DEVICE)
                .ok()
        } else {
            None
        };
        
        Self {
            no_draw,
            framebuffer,
        }
    }

    pub fn draw_text(&mut self, text: &str, position: (i32, i32), size: f32) -> Result<()> {
        // 使用 xochitl 的 dbus 接口来绘制文本
        // 这需要研究 xochitl 的 D-Bus API
        println!("尝试通过 xochitl 绘制文本: '{}'", text);
        
        // 临时方案：使用 remarkable-cli 工具
        let output = Command::new("remarkable-cli")
            .args(&["write", "--text", text])
            .output()?;
            
        if !output.status.success() {
            println!("绘制文本失败: {}", String::from_utf8_lossy(&output.stderr));
        }
        
        Ok(())
    }

    pub fn cleanup(&mut self) {
        // 不再需要重启 xochitl
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
