use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::fs::OpenOptions;
use std::io::{Write, Seek, SeekFrom};
use drm::control::{self, Device as ControlDevice};
use drm::Device;

// 先不设定具体的显示设备路径
pub struct Pen {
    no_draw: bool,
    drm_device: Option<std::fs::File>,
    width: u32,
    height: u32,
    buffer: Vec<u8>,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        let (drm_device, width, height) = if !no_draw {
            println!("尝试打开 DRM 设备: /dev/dri/card0");
            match OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/dri/card0")
            {
                Ok(device) => {
                    println!("成功打开 DRM 设备");
                    // 获取显示分辨率
                    let res = unsafe {
                        let dev = &device as &dyn Device;
                        dev.get_resources().unwrap()
                    };
                    
                    if let Some(connector) = res.connectors.first() {
                        let modes = unsafe {
                            let dev = &device as &dyn ControlDevice;
                            dev.get_connector(*connector, false).unwrap()
                        };
                        
                        if let Some(mode) = modes.modes().first() {
                            println!("显示分辨率: {}x{}", mode.size().0, mode.size().1);
                            (Some(device), mode.size().0, mode.size().1)
                        } else {
                            println!("无法获取显示模式，使用默认分辨率");
                            (Some(device), 1024, 600)
                        }
                    } else {
                        println!("无法获取显示连接器，使用默认分辨率");
                        (Some(device), 1024, 600)
                    }
                }
                Err(e) => {
                    println!("打开 DRM 设备失败: {}", e);
                    (None, 0, 0)
                }
            }
        } else {
            (None, 0, 0)
        };

        let buffer_size = (width * height) as usize;
        let buffer = vec![0u8; buffer_size];

        Self {
            no_draw,
            drm_device,
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

    pub fn cleanup(&mut self) {
        // 清理资源
    }

    fn draw_pixel(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        
        let offset = (y as u32 * self.width + x as u32) as usize;
        if offset < self.buffer.len() {
            self.buffer[offset] = 0xFF;
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
        if let Some(device) = &mut self.drm_device {
            // 将缓冲区内容刷新到屏幕
            unsafe {
                let dev = device as &dyn Device;
                // TODO: 实现 DRM 缓冲区刷新
                println!("刷新显示内容");
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
