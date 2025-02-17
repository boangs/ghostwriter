use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use nix::libc::{self, c_int, ioctl};
use nix::sys::mman::{mmap, MapFlags, ProtFlags};
use std::ptr;
use std::num::NonZeroUsize;
use nix::ioctl_write_int;

const REMARKABLE_WIDTH: u32 = 1404;
const REMARKABLE_HEIGHT: u32 = 1872;
const PEN_MAX_X: i32 = 15725;  // 触控笔 X 坐标最大值
const PEN_MAX_Y: i32 = 20967;  // 触控笔 Y 坐标最大值

// reMarkable 的 EPDC 更新结构
#[repr(C)]
struct MxcfbUpdateData {
    update_region: MxcfbRect,
    waveform_mode: u32,
    update_mode: u32,
    update_marker: u32,
    temp: i32,
    flags: u32,
    dither_mode: i32,
    quant_bit: i32,
    alt_buffer_data: MxcfbAltBufferData,
}

#[repr(C)]
struct MxcfbRect {
    top: u32,
    left: u32,
    width: u32,
    height: u32,
}

#[repr(C)]
struct MxcfbAltBufferData {
    phys_addr: u64,
    width: u32,
    height: u32,
    alt_update_region: MxcfbRect,
}

const REMARKABLE_WAVEFORM_MODE_DU: u32 = 1;
const REMARKABLE_UPDATE_MODE_PARTIAL: u32 = 0;
const MXCFB_SEND_UPDATE: u64 = 0x4044462E;  // 正确的 ioctl 命令号

// 尝试不同的显示设备路径
const FB_DEVICES: &[&str] = &[
    "/dev/fb0",
    "/dev/fb1",
    "/dev/qtfb",  // 可能的 Qt framebuffer 设备
    "/dev/remarkable_fb",  // 可能的自定义设备
];

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
        let display_device = if !no_draw {
            // 尝试打开所有可能的显示设备
            let mut device = None;
            for device_path in FB_DEVICES {
                println!("尝试打开显示设备: {}", device_path);
                match std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(device_path) 
                {
                    Ok(file) => {
                        println!("成功打开显示设备: {}", device_path);
                        device = Some(file);
                        break;
                    },
                    Err(e) => {
                        println!("打开显示设备 {} 失败: {}", device_path, e);
                    }
                }
            }
            device
        } else {
            None
        };

        let (framebuffer, buffer) = if let Some(ref display_device) = display_device {
            // 映射帧缓冲区
            let fb_size = REMARKABLE_WIDTH as usize * REMARKABLE_HEIGHT as usize * 2;
            let addr = unsafe {
                mmap(
                    None,
                    NonZeroUsize::new(fb_size).unwrap(),
                    ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                    MapFlags::MAP_SHARED,
                    display_device.as_raw_fd(),
                    0,
                )
            };
            
            match addr {
                Ok(ptr) => {
                    println!("成功映射帧缓冲区");
                    (Some(ptr as *mut u8), vec![0xFF; REMARKABLE_WIDTH as usize * REMARKABLE_HEIGHT as usize * 2])
                },
                Err(e) => {
                    println!("映射帧缓冲区失败: {}", e);
                    (None, vec![0xFF; REMARKABLE_WIDTH as usize * REMARKABLE_HEIGHT as usize * 2])
                }
            }
        } else {
            (None, vec![0xFF; REMARKABLE_WIDTH as usize * REMARKABLE_HEIGHT as usize * 2])
        };

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
                        let x = outline.min.x as i32 + 10 * x as i32;
                        let y = outline.min.y as i32 + 10 * y as i32;
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

    fn convert_coordinates(&self, x: i32, y: i32) -> (i32, i32) {
        // 调整坐标映射范围
        let display_x = (x as f32 * REMARKABLE_WIDTH as f32 / (PEN_MAX_X as f32 / 2.0)) as i32;
        let display_y = (y as f32 * REMARKABLE_HEIGHT as f32 / (PEN_MAX_Y as f32 / 2.0)) as i32;
        
        println!("坐标转换: 原始({}, {}) -> 显示({}, {})", 
            x, y, display_x, display_y);
            
        (display_x, display_y)
    }

    pub fn draw_point(&mut self, x: i32, y: i32, pressure: i32) -> Result<()> {
        // 转换坐标
        let (display_x, display_y) = self.convert_coordinates(x, y);
        
        if display_x < 0 || display_x >= REMARKABLE_WIDTH as i32 
            || display_y < 0 || display_y >= REMARKABLE_HEIGHT as i32 {
            return Ok(());
        }
        
        let index = (display_y as usize * REMARKABLE_WIDTH as usize + display_x as usize) * 2;
        if index + 1 >= self.buffer.len() {
            return Ok(());
        }
        
        // 将压力值（0-4095）转换为灰度值（0-15）
        let gray_level = match pressure {
            0 => 15,
            1..=4095 => {
                15 - ((pressure as f32 / 4095.0 * 15.0) as u8)
            },
            _ => 0,
        };
        
        let value = (gray_level as u16) * 0x1111;
        
        println!("绘制点 - 位置: ({}, {}), 压力: {}, 灰度: {}, 值: {:04X}", 
            display_x, display_y, pressure, gray_level, value);
        
        self.buffer[index] = (value & 0xFF) as u8;
        self.buffer[index + 1] = ((value >> 8) & 0xFF) as u8;
        
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if let Some(fb) = self.framebuffer {
            unsafe {
                ptr::copy_nonoverlapping(
                    self.buffer.as_ptr(),
                    fb,
                    self.buffer.len()
                );
                
                if let Some(device) = &self.display_device {
                    let update_data = MxcfbUpdateData {
                        update_region: MxcfbRect {
                            top: 0,
                            left: 0,
                            width: REMARKABLE_WIDTH,
                            height: REMARKABLE_HEIGHT,
                        },
                        waveform_mode: REMARKABLE_WAVEFORM_MODE_DU,
                        update_mode: REMARKABLE_UPDATE_MODE_PARTIAL,
                        update_marker: 0,
                        temp: 25,
                        flags: 0,
                        dither_mode: 0,
                        quant_bit: 0,
                        alt_buffer_data: MxcfbAltBufferData {
                            phys_addr: 0,
                            width: 0,
                            height: 0,
                            alt_update_region: MxcfbRect {
                                top: 0,
                                left: 0,
                                width: 0,
                                height: 0,
                            },
                        },
                    };

                    let fd = device.as_raw_fd();
                    match ioctl(fd, MXCFB_SEND_UPDATE, &update_data as *const _) {
                        -1 => println!("发送刷新命令失败: {}", std::io::Error::last_os_error()),
                        _ => println!("发送刷新命令成功"),
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
