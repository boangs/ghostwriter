use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use nix::libc;
use std::os::unix::io::AsRawFd;

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
            let devices = ["/dev/dri/card0", "/dev/fb0", "/dev/graphics/fb0"];
            let mut device_info = None;
            
            for device_path in devices.iter() {
                println!("尝试打开显示设备: {}", device_path);
                if let Ok(device) = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(libc::O_RDWR)
                    .open(device_path) 
                {
                    println!("成功打开显示设备: {}", device_path);
                    let fd = device.as_raw_fd();
                    println!("显示设备文件描述符: {}", fd);
                    device_info = Some((Some(device), 1024, 600));
                    break;
                }
            }
            
            device_info.unwrap_or_else(|| {
                println!("所有显示设备都打开失败");
                (None, 0, 0)
            })
        } else {
            (None, 0, 0)
        };

        let buffer_size = (width * height * 4) as usize;  // 32位色深
        println!("创建显示缓冲区，大小: {} 字节", buffer_size);
        let buffer = vec![0xFF; buffer_size];

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
        
        let offset = ((y as u32 * self.width + x as u32) * 4) as usize;
        if offset + 3 < self.buffer.len() {
            // RGBA 格式，设置为黑色
            self.buffer[offset] = 0;     // R
            self.buffer[offset+1] = 0;   // G
            self.buffer[offset+2] = 0;   // B
            self.buffer[offset+3] = 255; // A
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
            println!("缓冲区大小: {} 字节", self.buffer.len());
            match device.write_all(&self.buffer) {
                Ok(_) => println!("写入缓冲区成功"),
                Err(e) => println!("写入缓冲区失败: {}", e)
            }
            match device.flush() {
                Ok(_) => println!("刷新显示成功"),
                Err(e) => println!("刷新显示失败: {}", e)
            }
        }
        Ok(())
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        self.cleanup();
    }
}
