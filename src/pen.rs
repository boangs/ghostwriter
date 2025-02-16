use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use nix::libc;

const REMARKABLE_WIDTH: u32 = 1404;
const REMARKABLE_HEIGHT: u32 = 1872;

pub struct Pen {
    no_draw: bool,
    display_device: Option<File>,
    pen_device: Option<File>,
    width: u32,
    height: u32,
    buffer: Vec<u8>,
    last_x: i32,
    last_y: i32,
    pressure: i32,
    is_drawing: bool,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        let display_device = if !no_draw {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_SYNC)
                .open("/dev/fb0") 
            {
                Ok(file) => {
                    println!("成功打开显示设备，fd: {}", file.as_raw_fd());
                    Some(file)
                },
                Err(e) => {
                    println!("打开显示设备失败: {} (errno={})", 
                        e, e.raw_os_error().unwrap_or(-1));
                    None
                }
            }
        } else {
            None
        };

        let buffer = vec![0xFF; REMARKABLE_WIDTH as usize * REMARKABLE_HEIGHT as usize * 2];
        
        Self {
            no_draw,
            display_device,
            pen_device: None,
            width: REMARKABLE_WIDTH,
            height: REMARKABLE_HEIGHT,
            buffer,
            last_x: 0,
            last_y: 0,
            pressure: 0,
            is_drawing: false,
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
                        let _ = self.draw_point(x, y, 2000);
                    }
                });
            }
        }
        self.flush()?;
        println!("文本绘制完成");
        Ok(())
    }

    pub fn handle_pen_input(&mut self) -> Result<()> {
        // 从 /dev/input/event2 读取笔输入
        let device = "/dev/input/event2";  // Elan marker input
        
        // 打开设备
        let mut input_device = File::open(device)?;
        
        // 读取事件
        let mut event_buf = [0u8; 24];
        input_device.read_exact(&mut event_buf)?;
        
        // 解析事件
        let event = parse_input_event(&event_buf);
        
        // 根据事件类型处理坐标
        match (event.type_, event.code) {
            (3, 0) => self.last_x = event.value, // X 坐标
            (3, 1) => self.last_y = event.value, // Y 坐标
            _ => return Ok(()),
        }
        
        // 绘制点
        self.draw_point(self.last_x, self.last_y, self.pressure)?;
        self.flush()?;
        
        Ok(())
    }

    pub fn draw_point(&mut self, x: i32, y: i32, pressure: i32) -> Result<()> {
        if x < 0 || x >= REMARKABLE_WIDTH as i32 || y < 0 || y >= REMARKABLE_HEIGHT as i32 {
            return Ok(());
        }
        
        let index = (y as usize * REMARKABLE_WIDTH as usize + x as usize) * 2;
        if index + 1 >= self.buffer.len() {
            return Ok(());
        }
        
        // 根据压力值计算颜色深度 (0-255)
        let color = match pressure {
            0 => 255,  // 无压力 = 白色
            1..=255 => 255 - pressure,  // 压力越大越黑
            _ => 0,  // 最大压力 = 黑色
        };
        
        self.buffer[index] = color as u8;
        self.buffer[index + 1] = color as u8;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if let Some(device) = &mut self.display_device {
            println!("开始写入显示缓冲区，大小: {} 字节", self.buffer.len());
            
            match device.write_all(&self.buffer) {
                Ok(_) => {
                    println!("缓冲区写入成功");
                    match device.sync_all() {
                        Ok(_) => println!("缓冲区同步成功"),
                        Err(e) => println!("缓冲区同步失败: {} (errno={})", 
                            e, e.raw_os_error().unwrap_or(-1))
                    }
                },
                Err(e) => {
                    println!("缓冲区写入失败: {} (errno={})", 
                        e, e.raw_os_error().unwrap_or(-1));
                        
                    println!("尝试重新打开显示设备");
                    self.display_device = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .custom_flags(libc::O_SYNC)  // 添加同步标志
                        .open("/dev/fb0")
                        .map_err(|e| {
                            println!("重新打开显示设备失败: {} (errno={})",
                                e, e.raw_os_error().unwrap_or(-1));
                            e
                        })
                        .ok();
                    
                    if let Some(new_device) = &mut self.display_device {
                        println!("显示设备重新打开成功，尝试写入");
                        new_device.write_all(&self.buffer)?;
                        new_device.sync_all()?;
                        println!("显示设备刷新完成");
                    }
                }
            }
        } else {
            println!("警告：显示设备未初始化");
        }
        Ok(())
    }

    pub fn draw_bitmap(&mut self, bitmap: &[u8]) -> Result<()> {
        if bitmap.len() != self.buffer.len() {
            return Err(anyhow::anyhow!("位图大小不匹配"));
        }
        self.buffer.copy_from_slice(bitmap);
        self.flush()
    }
}

fn parse_input_event(buffer: &[u8]) -> InputEvent {
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

#[derive(Debug)]
struct InputEvent {
    time: libc::timeval,
    type_: u16,
    code: u16,
    value: i32,
}
