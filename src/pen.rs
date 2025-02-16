use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use nix::libc;
use nix::sys::mman::{mmap, MapFlags, ProtFlags};
use std::ptr;
use std::num::NonZeroUsize;
use nix::ioctl_write_int;

const REMARKABLE_WIDTH: u32 = 1404;
const REMARKABLE_HEIGHT: u32 = 1872;

// 定义 EPDC 刷新命令的 ioctl 号
ioctl_write_int!(mxcfb_send_update, b'F', 0x2E, i32);

pub struct Pen {
    no_draw: bool,
    display_device: Option<File>,
    framebuffer: Option<*mut u8>,
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
        let (display_device, framebuffer) = if !no_draw {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_SYNC)
                .open("/dev/fb0") 
            {
                Ok(file) => {
                    println!("成功打开显示设备，fd: {}", file.as_raw_fd());
                    
                    // 映射帧缓冲区
                    let fb_size = REMARKABLE_WIDTH as usize * REMARKABLE_HEIGHT as usize * 2;
                    let addr = unsafe {
                        mmap(
                            None,
                            NonZeroUsize::new(fb_size).unwrap(),
                            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                            MapFlags::MAP_SHARED,
                            file.as_raw_fd(),
                            0,
                        )
                    };
                    
                    match addr {
                        Ok(ptr) => {
                            println!("成功映射帧缓冲区");
                            (Some(file), Some(ptr as *mut u8))
                        },
                        Err(e) => {
                            println!("映射帧缓冲区失败: {}", e);
                            (Some(file), None)
                        }
                    }
                },
                Err(e) => {
                    println!("打开显示设备失败: {} (errno={})", 
                        e, e.raw_os_error().unwrap_or(-1));
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        let buffer = vec![0xFF; REMARKABLE_WIDTH as usize * REMARKABLE_HEIGHT as usize * 2];
        
        Self {
            no_draw,
            display_device,
            framebuffer,
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
        
        let gray_level = match pressure {
            0 => 15,
            1..=4095 => {
                15 - ((pressure as f32 / 4095.0 * 15.0) as u8)
            },
            _ => 0,
        };
        
        let value = (gray_level as u16) * 0x1111;
        
        // 添加调试信息
        println!("绘制点 - 位置: ({}, {}), 压力: {}, 灰度: {}, 值: {:04X}", 
            x, y, pressure, gray_level, value);
        
        // 检查缓冲区当前值
        let old_value = ((self.buffer[index + 1] as u16) << 8) | (self.buffer[index] as u16);
        println!("缓冲区原值: {:04X}", old_value);
        
        self.buffer[index] = (value & 0xFF) as u8;
        self.buffer[index + 1] = ((value >> 8) & 0xFF) as u8;
        
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if let Some(fb) = self.framebuffer {
            unsafe {
                // 复制缓冲区内容到帧缓冲区
                ptr::copy_nonoverlapping(
                    self.buffer.as_ptr(),
                    fb,
                    self.buffer.len()
                );
                
                // 如果有显示设备，发送刷新命令
                if let Some(device) = &self.display_device {
                    let fd = device.as_raw_fd();
                    match mxcfb_send_update(fd, 0) {
                        Ok(_) => println!("发送刷新命令成功"),
                        Err(e) => println!("发送刷新命令失败: {}", e)
                    }
                }
            }
            println!("帧缓冲区更新完成");
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

    pub fn test_display(&mut self) -> Result<()> {
        // 将整个屏幕变成黑色
        for i in 0..self.buffer.len() {
            self.buffer[i] = 0x00;
        }
        self.flush()?;
        
        // 等待 1 秒
        std::thread::sleep(std::time::Duration::from_secs(1));
        
        // 将整个屏幕变成白色
        for i in 0..self.buffer.len() {
            self.buffer[i] = 0xFF;
        }
        self.flush()?;
        
        Ok(())
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        if let Some(fb) = self.framebuffer {
            unsafe {
                let _ = nix::sys::mman::munmap(
                    fb as *mut std::ffi::c_void,
                    self.buffer.len()
                );
            }
        }
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
