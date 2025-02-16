use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::fs::OpenOptions;
use std::io::{Write, Seek, SeekFrom};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use nix::libc;
use drm::control::{Device as ControlDevice, connector, crtc};
use drm::Device;
use std::thread::sleep;
use std::time::Duration;

pub struct Pen {
    no_draw: bool,
    display_device: Option<std::fs::File>,
    width: u32,
    height: u32,
    buffer: Vec<u8>,
    fb_id: Option<u32>,
    crtc_id: Option<u32>,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        let (display_device, width, height, fb_id, crtc_id) = if !no_draw {
            println!("尝试打开显示设备: /dev/dri/card0");
            match OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_RDWR)
                .open("/dev/dri/card0")
            {
                Ok(device) => {
                    println!("成功打开显示设备");
                    let dev = &device as &dyn Device;
                    
                    // 获取资源
                    let res = unsafe { dev.get_resources().unwrap() };
                    let conn = res.connectors[0];
                    let crtc = res.crtcs[0];
                    
                    // 获取连接器信息
                    let conn_info = unsafe {
                        device.get_connector(conn, false).unwrap()
                    };
                    
                    // 创建帧缓冲区
                    let fb_id = unsafe {
                        device.add_framebuffer(
                            1024, 600,
                            32, 32,
                            1024 * 4,
                            0,
                            &[],
                        ).unwrap()
                    };
                    
                    (Some(device), 1024, 600, Some(fb_id), Some(crtc))
                }
                Err(e) => {
                    println!("打开显示设备失败: {}", e);
                    (None, 0, 0, None, None)
                }
            }
        } else {
            (None, 0, 0, None, None)
        };

        let buffer_size = (width * height * 4) as usize;
        let buffer = vec![0xFF; buffer_size];

        Self {
            no_draw,
            display_device,
            width,
            height,
            buffer,
            fb_id,
            crtc_id,
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
            self.buffer[offset] = 0;
            self.buffer[offset + 1] = 0;
            self.buffer[offset + 2] = 0;
            self.buffer[offset + 3] = 255;
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
        if let (Some(device), Some(fb_id), Some(crtc_id)) = 
            (&mut self.display_device, self.fb_id, self.crtc_id) {
            println!("开始刷新显示");
            
            unsafe {
                let dev = device as &dyn Device;
                // 更新帧缓冲区内容
                dev.map_dumb_buffer(fb_id, &self.buffer)?;
                
                // 设置 CRTC
                let mode = crtc::Mode::new(
                    self.width, self.height,
                    60  // 刷新率
                );
                dev.set_crtc(crtc_id, Some(fb_id), 0, 0, &[fb_id], Some(mode))?;
            }
            
            println!("显示刷新完成");
        }
        Ok(())
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        if let (Some(device), Some(fb_id)) = (&self.display_device, self.fb_id) {
            unsafe {
                let dev = device as &dyn Device;
                let _ = dev.destroy_framebuffer(fb_id);
            }
        }
        self.cleanup();
    }
}
