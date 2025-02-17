use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use nix::libc::{self, c_int, ioctl, ftruncate};
use nix::sys::mman::{mmap, MapFlags, ProtFlags, shm_open, shm_unlink};
use std::ptr;
use std::num::NonZeroUsize;
use nix::ioctl_write_int;
use std::ffi::CString;
use std::os::unix::io::RawFd;
use nix::fcntl::OFlag;
use nix::sys::stat::Mode;

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
const MXCFB_SEND_UPDATE: u64 = 0x4048462e;  // 正确的 ioctl 命令号

// 修改显示设备路径
const FB_DEVICES: &[&str] = &[
    "/dev/fb0",  // rMPP 主显示设备
];

// rMPP 的更新命令
const RMPP_UPDATE_DISPLAY: u64 = 0x5730;

// 修改共享内存路径
const SHMEM_PATH: &str = "/rmpp-qtfb";
const SCREEN_SIZE: usize = (REMARKABLE_WIDTH * REMARKABLE_HEIGHT * 4) as usize;

// 每个像素的位深度
const BITS_PER_PIXEL: u32 = 32;
const BYTES_PER_PIXEL: u32 = BITS_PER_PIXEL / 8;

// 帧缓冲区相关常量和结构体定义
const FBIOGET_VSCREENINFO: u64 = 0x4600;
const FBIOGET_FSCREENINFO: u64 = 0x4602;

#[repr(C)]
struct FbVarScreeninfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

#[repr(C)]
struct FbFixScreeninfo {
    id: [u8; 16],
    smem_start: u64,
    smem_len: u32,
    type_: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: u64,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

#[repr(C)]
struct FbBitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

pub struct Pen {
    no_draw: bool,
    shmem_fd: Option<RawFd>,
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
    pub fn new(no_draw: bool) -> Result<Self> {
        let (framebuffer, shmem_fd) = if !no_draw {
            let fd = unsafe {
                shm_open(
                    SHMEM_PATH,
                    OFlag::O_RDWR | OFlag::O_CREAT,
                    Mode::from_bits_truncate(0o644)
                )?
            };
            
            // 处理 ftruncate 的返回值
            unsafe { 
                if ftruncate(fd, SCREEN_SIZE as i64) == -1 {
                    return Err(anyhow::anyhow!("Failed to set shared memory size"));
                }
            };
            
            let addr = unsafe {
                mmap(
                    None,
                    NonZeroUsize::new(SCREEN_SIZE).unwrap(),
                    ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                    MapFlags::MAP_SHARED,
                    fd,
                    0
                )?
            };
            
            (Some(addr as *mut u8), Some(fd))
        } else {
            (None, None)
        };

        let pen_device = if !no_draw {
            // 尝试打开所有可能的显示设备
            let mut device = None;
            for device_path in FB_DEVICES {
                println!("尝试打开显示设备: {}", device_path);
                match std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(device_path) 
                {
                    Ok(mut file) => {
                        println!("成功打开显示设备: {}", device_path);
                        
                        // 获取帧缓冲区信息
                        let mut var_info: FbVarScreeninfo = unsafe { std::mem::zeroed() };
                        let mut fix_info: FbFixScreeninfo = unsafe { std::mem::zeroed() };
                        
                        unsafe {
                            let fd = file.as_raw_fd();
                            if ioctl(fd, FBIOGET_VSCREENINFO, &mut var_info as *mut _) >= 0 {
                                println!("帧缓冲区可变信息:");
                                println!("  分辨率: {}x{}", var_info.xres, var_info.yres);
                                println!("  位深度: {}", var_info.bits_per_pixel);
                                println!("  颜色格式: R{}G{}B{}, A{}", 
                                    var_info.red.length, 
                                    var_info.green.length,
                                    var_info.blue.length,
                                    var_info.transp.length);
                            }
                            
                            if ioctl(fd, FBIOGET_FSCREENINFO, &mut fix_info as *mut _) >= 0 {
                                println!("帧缓冲区固定信息:");
                                println!("  内存大小: {} 字节", fix_info.smem_len);
                                println!("  行长度: {} 字节", fix_info.line_length);
                            }
                        }
                        
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

        Ok(Self {
            no_draw,
            shmem_fd,
            framebuffer,
            pen_device,
            width: REMARKABLE_WIDTH,
            height: REMARKABLE_HEIGHT,
            buffer: vec![0xFF; SCREEN_SIZE],
            last_x: 0,
            last_y: 0,
            pressure: 0,
            is_drawing: false,
        })
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
        
        let index = (display_y as usize * REMARKABLE_WIDTH as usize + display_x as usize) * BYTES_PER_PIXEL as usize;
        if index + 3 >= self.buffer.len() {
            return Ok(());
        }
        
        // 修改灰度值计算
        let gray_level = match pressure {
            0 => 255,
            1..=4095 => {
                255 - ((pressure as f32 / 4095.0 * 255.0) as u8)
            },
            _ => 0,
        };
        
        // RGBA 格式
        self.buffer[index] = gray_level;     // R
        self.buffer[index + 1] = gray_level; // G
        self.buffer[index + 2] = gray_level; // B
        self.buffer[index + 3] = 255;        // A
        
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if let Some(ref mut device) = self.pen_device {
            // 移动到文件开始
            device.seek(SeekFrom::Start(0))?;
            
            // 直接写入帧缓冲区
            device.write_all(&self.buffer)?;
            println!("直接写入帧缓冲区完成");
            
            // 尝试强制刷新
            device.sync_all()?;
            println!("帧缓冲区同步完成");
        } else {
            println!("未找到显示设备");
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
        println!("开始显示测试...");
        
        // 1. 清屏为白色
        for i in 0..self.buffer.len() {
            self.buffer[i] = 0xFF;
        }
        self.flush()?;
        println!("清屏完成");
        
        std::thread::sleep(std::time::Duration::from_secs(1));
        
        // 2. 绘制一个黑色矩形
        let rect_x = 100;
        let rect_y = 100;
        let rect_width = 200;
        let rect_height = 200;
        
        for y in rect_y..rect_y+rect_height {
            for x in rect_x..rect_x+rect_width {
                let index = (y as usize * REMARKABLE_WIDTH as usize + x as usize) * BYTES_PER_PIXEL as usize;
                if index + 3 < self.buffer.len() {
                    self.buffer[index] = 0x00;
                    self.buffer[index + 1] = 0x00;
                    self.buffer[index + 2] = 0x00;
                    self.buffer[index + 3] = 255;
                }
            }
        }
        self.flush()?;
        println!("绘制矩形完成");
        
        std::thread::sleep(std::time::Duration::from_secs(1));
        
        // 3. 绘制测试文本
        self.draw_text("测试显示功能", (300, 300), 32.0)?;
        println!("绘制文本完成");
        
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
        
        if let Some(fd) = self.shmem_fd {
            unsafe {
                let _ = shm_unlink(SHMEM_PATH);  // 直接使用字符串字面量
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

#[repr(C)]
struct RmppUpdateData {
    update_region: MxcfbRect,
    update_mode: u32,
    flags: u32,
}
