use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use nix::libc;
use nix::ioctl_read;
use std::os::unix::io::AsRawFd;

ioctl_read!(fb_var_screeninfo, b'F', 0, winsize);

pub struct Pen {
    no_draw: bool,
    display_device: Option<std::fs::File>,
    width: u32,
    height: u32,
    buffer: Vec<u8>,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        let (display_device, width, height) = if !no_draw {
            println!("尝试打开显示设备: /dev/fb0");
            match OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_RDWR)
                .open("/dev/fb0")
            {
                Ok(device) => {
                    println!("成功打开显示设备");
                    let mut info: winsize = unsafe { std::mem::zeroed() };
                    unsafe {
                        if fb_var_screeninfo(device.as_raw_fd(), &mut info as *mut _).is_ok() {
                            println!("显示信息: {}x{}", info.ws_col, info.ws_row);
                        }
                    }
                    (Some(device), info.ws_col as u32, info.ws_row as u32)
                }
                Err(e) => {
                    println!("打开显示设备失败: {}", e);
                    (None, 0, 0)
                }
            }
        } else {
            (None, 0, 0)
        };

        let buffer_size = (width * height) as usize;
        let buffer = vec![0xFF; buffer_size];  // 白色背景

        Self {
            no_draw,
            display_device,
            width,
            height,
            buffer,
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
        
        println!("开始绘制文本: {}", text);
        for glyph in glyphs {
            if let Some(outline) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    if v > 0.1 {
                        let x = outline.min.x as i32 + x as i32;
                        let y = outline.min.y as i32 + y as i32;
                        self.draw_pixel(x, y);
                        println!("绘制点: ({}, {})", x, y);
                    }
                });
            }
        }
        self.flush()?;
        println!("文本绘制完成");
        
        Ok(())
    }

    pub fn cleanup(&mut self) {
        // 清理资源
    }

    fn draw_pixel(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        
        let offset = (y as u32 * self.width + x as u32) as usize;
        if offset < self.buffer.len() {
            self.buffer[offset] = 0;  // 黑色像素
        }
    }

    pub fn draw_bitmap(&mut self, bitmap: &Vec<Vec<bool>>) -> Result<()> {
        for (y, row) in bitmap.iter().enumerate() {
            for (x, &pixel) in row.iter().enumerate() {
                if pixel {
                    self.draw_pixel(x as i32, y as i32);
                }
            }
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if let Some(device) = &mut self.display_device {
            println!("开始刷新显示");
            device.write_all(&self.buffer)?;
            device.flush()?;
            println!("显示刷新完成");
        }
        Ok(())
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        self.cleanup();
    }
}
