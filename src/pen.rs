use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use nix::libc;
use std::os::unix::io::AsRawFd;
use std::fs::File;
use std::io::{Read, Write};

pub struct Pen {
    no_draw: bool,
    display_device: Option<File>,
    pen_device: Option<File>,
    width: u32,
    height: u32,
    buffer: Vec<u8>,
    is_drawing: bool,
    last_y: i32,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        let (display_device, pen_device, width, height) = if !no_draw {
            // 打开显示设备
            let display = File::options()
                .read(true)
                .write(true)
                .custom_flags(libc::O_RDWR)
                .open("/dev/fb0")
                .ok();

            // 打开手写笔输入设备
            let pen = File::options()
                .read(true)
                .open("/dev/input/event2")  // Elan marker input
                .ok();

            (display, pen, 2832, 2064)  // reMarkable Paper Pro 分辨率
        } else {
            (None, None, 0, 0)
        };

        let buffer_size = (width * height) as usize;
        let buffer = vec![255u8; buffer_size];  // 白色背景

        Self {
            no_draw,
            display_device,
            pen_device,
            width,
            height, 
            buffer,
            is_drawing: false,
            last_y: 0,
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

    pub fn handle_pen_input(&mut self) -> Result<()> {
        if let Some(pen_device) = &mut self.pen_device {
            let mut event_buffer = [0u8; 24];  // Linux input_event 结构体大小
            
            while let Ok(_) = pen_device.read_exact(&mut event_buffer) {
                let event = parse_input_event(&event_buffer);
                
                match event.type_ {
                    0 => {}, // EV_SYN
                    3 => {   // EV_ABS
                        match event.code {
                            0 => {  // ABS_X
                                let x = event.value as i32;
                                let y = self.last_y;
                                self.draw_pixel(x, y);
                            },
                            1 => {  // ABS_Y 
                                self.last_y = event.value as i32;
                            },
                            24 => { // ABS_PRESSURE
                                if event.value > 0 {
                                    self.is_drawing = true;
                                } else {
                                    self.is_drawing = false;
                                }
                            },
                            _ => {}
                        }
                    },
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn draw_pixel(&mut self, x: i32, y: i32) {
        if !self.is_drawing {
            return;
        }
        
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        
        let offset = (y as u32 * self.width + x as u32) as usize;
        if offset < self.buffer.len() {
            self.buffer[offset] = 0;  // 黑色像素
        }
        
        self.flush().unwrap_or_default();
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
            device.write_all(&self.buffer)?;
            device.flush()?;
        }
        Ok(())
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[derive(Debug)]
struct InputEvent {
    time: libc::timeval,
    type_: u16,
    code: u16,
    value: i32,
}

fn parse_input_event(buffer: &[u8]) -> InputEvent {
    // 解析 Linux input_event 结构体
    let time_sec = u64::from_ne_bytes(buffer[0..8].try_into().unwrap());
    let time_usec = u64::from_ne_bytes(buffer[8..16].try_into().unwrap());
    let type_ = u16::from_ne_bytes(buffer[16..18].try_into().unwrap());
    let code = u16::from_ne_bytes(buffer[18..20].try_into().unwrap());
    let value = i32::from_ne_bytes(buffer[20..24].try_into().unwrap());

    InputEvent {
        time: libc::timeval {
            tv_sec: time_sec as i64,
            tv_usec: time_usec as i64,
        },
        type_,
        code,
        value,
    }
}
