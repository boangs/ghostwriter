use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::fs::MetadataExt;
use nix::libc::{self, c_int, ioctl, ftruncate};
use nix::sys::mman::{mmap, MapFlags, ProtFlags, shm_open, shm_unlink};
use std::ptr;
use std::num::NonZeroUsize;
use nix::ioctl_write_int;
use std::ffi::CString;
use std::os::unix::io::RawFd;
use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use drm::{Device as DrDevice, control::Device};
use drm::control::{connector, crtc, framebuffer, Mode};
use drm::buffer::DrmFourcc;

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
    "/dev/dri/card0",  // DRM 设备
    "/dev/fb0",        // 传统帧缓冲设备作为备选
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
    drm_device: Option<std::fs::File>,
    framebuffer: Option<framebuffer::Handle>,
    crtc: Option<crtc::Handle>,
    connector: Option<connector::Handle>,
    mode: Option<Mode>,
    buffer: Vec<u8>,
    width: u32,
    height: u32,
    last_x: i32,
    last_y: i32,
    pressure: i32,
    is_drawing: bool,
}

impl Pen {
    pub fn new(no_draw: bool) -> Result<Self> {
        let (drm_device, framebuffer, crtc, connector, mode) = if !no_draw {
            println!("尝试打开显示设备: {}", "/dev/dri/card0");
            let drm_device = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open("/dev/dri/card0")?;

            // 获取可用的连接器
            let res_handles = drm_device.resource_handles()?;
            let connector = res_handles.connectors()
                .iter()
                .find(|&&conn_handle| {
                    if let Ok(info) = drm_device.get_connector(conn_handle, false) {
                        info.state() == connector::State::Connected
                    } else {
                        false
                    }
                })
                .copied()
                .ok_or_else(|| anyhow::anyhow!("没有找到已连接的显示器"))?;

            // 获取连接器信息
            let connector_info = drm_device.get_connector(connector, false)?;
            let mode = connector_info.modes()[0];  // 使用第一个可用的显示模式

            // 获取编码器
            let encoder = connector_info.current_encoder()
                .ok_or_else(|| anyhow::anyhow!("没有找到编码器"))?;

            // 获取 CRTC
            let crtc = drm_device.get_encoder(encoder)?
                .crtc()
                .ok_or_else(|| anyhow::anyhow!("没有找到 CRTC"))?;

            // 创建帧缓冲区
            let fb_id = drm_device.create_framebuffer(&[0u8; SCREEN_SIZE], 
                REMARKABLE_WIDTH, 
                REMARKABLE_HEIGHT,
                DrmFourcc::Xrgb8888,
                &[REMARKABLE_WIDTH * 4],
                &[0],
            )?;

            println!("成功初始化 DRM 设备");
            (Some(drm_device), Some(fb_id), Some(crtc), Some(connector), Some(mode))
        } else {
            (None, None, None, None, None)
        };

        Ok(Self {
            no_draw,
            drm_device,
            framebuffer,
            crtc,
            connector,
            mode,
            buffer: vec![0xFF; SCREEN_SIZE],
            width: REMARKABLE_WIDTH,
            height: REMARKABLE_HEIGHT,
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
        
        // 使用单一灰度值
        let pixel_value = gray_level;
        for i in 0..BYTES_PER_PIXEL as usize {
            self.buffer[index + i] = pixel_value;
        }
        
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if let (Some(ref mut device), Some(fb), Some(crtc), Some(mode)) = 
            (&mut self.drm_device, self.framebuffer, self.crtc, self.mode) {
            // 更新帧缓冲区内容
            device.add_fb(&self.buffer, 
                REMARKABLE_WIDTH, 
                REMARKABLE_HEIGHT,
                DrmFourcc::Xrgb8888,
                &[REMARKABLE_WIDTH * 4],
                &[0],
            )?;

            // 设置 CRTC
            device.set_crtc(crtc, Some(fb), (0, 0), &[self.connector.unwrap()], Some(mode))?;
            println!("显示更新完成");
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
        
        // 2. 绘制一个简单的黑色条纹图案
        for y in 0..REMARKABLE_HEIGHT {
            for x in 0..REMARKABLE_WIDTH {
                let index = (y as usize * REMARKABLE_WIDTH as usize + x as usize) * BYTES_PER_PIXEL as usize;
                if y % 100 < 50 {  // 每100像素绘制50像素宽的黑色条纹
                    for i in 0..BYTES_PER_PIXEL as usize {
                        self.buffer[index + i] = 0x00;
                    }
                }
            }
        }
        self.flush()?;
        println!("绘制条纹图案完成");
        
        Ok(())
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        if let (Some(ref mut device), Some(fb)) = (&mut self.drm_device, self.framebuffer) {
            // 清理帧缓冲区
            if let Err(e) = device.destroy_framebuffer(fb) {
                eprintln!("清理帧缓冲区失败: {}", e);
            }
        }
        // DRM 设备会在 File 被 drop 时自动关闭
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
