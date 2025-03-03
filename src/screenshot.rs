use anyhow::Result;
use image::{GrayImage, DynamicImage, ImageBuffer, Luma};
use log::{info, error};
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom, BufRead, BufReader};
use std::os::unix::io::AsRawFd;
use std::process::Command;
use crate::constants::{REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use std::mem::size_of;

use base64::{engine::general_purpose, Engine as _};
use image::ImageEncoder;

const WIDTH: usize = 1404;  // remarkable paper pro 的实际宽度
const HEIGHT: usize = 1872; // remarkable paper pro 的实际高度
const BYTES_PER_PIXEL: usize = 4;  // RGBA 格式
const WINDOW_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;

// reMarkable 显示内存的可能物理地址
const DISPLAY_MEM_ADDRS: [u64; 3] = [
    0x20000000,  // 第一个可能的地址
    0x9C000000,  // 第二个可能的地址
    0x10000000,  // 第三个可能的地址
];
const DISPLAY_MEM_SIZE: usize = WINDOW_BYTES;

const OUTPUT_WIDTH: u32 = 768;
const OUTPUT_HEIGHT: u32 = 1024;

// DRM ioctl 命令和结构体定义
const DRM_IOCTL_MODE_MAP_DUMB: u64 = 0xC01064B2;
const DRM_IOCTL_MODE_CREATE_DUMB: u64 = 0xC01064B0;

#[repr(C)]
struct DrmModeModeInfo {
    clock: u32,
    hdisplay: u16,
    hsync_start: u16,
    hsync_end: u16,
    htotal: u16,
    vdisplay: u16,
    vsync_start: u16,
    vsync_end: u16,
    vtotal: u16,
    flags: u32,
}

#[repr(C)]
struct DrmModeCreateDumb {
    height: u32,
    width: u32,
    bpp: u32,
    flags: u32,
    handle: u32,
    pitch: u32,
    size: u64,
}

#[repr(C)]
struct DrmModeMapDumb {
    handle: u32,
    pad: u32,
    offset: u64,
}

pub struct Screenshot {
    data: Vec<u8>,
}

impl Screenshot {
    pub fn new() -> Result<Screenshot> {
        let screenshot_data = Self::take_screenshot()?;
        Ok(Screenshot {
            data: screenshot_data,
        })
    }

    fn take_screenshot() -> Result<Vec<u8>> {
        // 获取 xochitl 进程的 PID
        let output = Command::new("pidof")
            .arg("xochitl")
            .output()?;
        let pid = String::from_utf8(output.stdout)?.trim().to_string();
        info!("找到 xochitl 进程 PID: {}", pid);

        // 读取内存映射
        let maps_path = format!("/proc/{}/maps", pid);
        let maps_file = File::open(&maps_path)?;
        let reader = BufReader::new(maps_file);
        let mut lines: Vec<String> = reader.lines().collect::<std::io::Result<_>>()?;
        lines.reverse();

        // 查找 /dev/dri/card0 相关的内存区域
        let mut memory_range = None;
        for i in 0..lines.len() {
            if lines[i].contains("/dev/dri/card0") {
                if i > 0 {
                    let range = lines[i-1].split_whitespace().next().unwrap();
                    memory_range = Some(range.to_string());
                    break;
                }
            }
        }

        let range = memory_range.ok_or_else(|| anyhow::anyhow!("未找到显示内存区域"))?;
        let (start_str, end_str) = range.split_once('-').unwrap();
        let start = u64::from_str_radix(start_str, 16)?;
        let end = u64::from_str_radix(end_str, 16)?;
        
        info!("找到内存区域: {} (start: 0x{:x}, end: 0x{:x})", range, start, end);

        // 打开进程内存
        let mut mem_file = File::open(format!("/proc/{}/mem", pid))?;
        
        // 查找实际的显示内存位置
        let mut offset = 0u64;
        let mut length = 2u64;
        
        while length < (WIDTH * HEIGHT * 4) as u64 {
            offset += length - 2;
            mem_file.seek(SeekFrom::Start(start + offset + 8))?;
            
            let mut header = [0u8; 8];
            mem_file.read_exact(&mut header)?;
            length = u64::from_le_bytes(header);
        }

        let skip = start + offset;
        info!("找到显示内存偏移量: 0x{:x}", skip);

        // 读取显示内存
        mem_file.seek(SeekFrom::Start(skip))?;
        let mut buffer = vec![0u8; WINDOW_BYTES];
        mem_file.read_exact(&mut buffer)?;

        info!("读取显示内存成功，大小: {} 字节", buffer.len());

        // 处理图像数据
        let processed_data = Self::process_image(buffer)?;

        Ok(processed_data)
    }

    fn process_image(data: Vec<u8>) -> Result<Vec<u8>> {
        // 将 RGBA 数据转换为灰度图
        let mut gray_data = vec![0u8; WIDTH * HEIGHT];
        for i in 0..WIDTH * HEIGHT {
            let rgba = &data[i * 4..(i + 1) * 4];
            // 使用加权平均值计算灰度值，增加对比度
            gray_data[i] = ((rgba[0] as f32 * 0.299 + 
                           rgba[1] as f32 * 0.587 + 
                           rgba[2] as f32 * 0.114) * 1.2) as u8;
        }

        let img = GrayImage::from_raw(WIDTH as u32, HEIGHT as u32, gray_data)
            .ok_or_else(|| anyhow::anyhow!("无法从原始数据创建图像"))?;

        // 增强对比度
        let enhanced = imageproc::contrast::stretch_contrast(&img);
        
        // 编码为 PNG
        let mut png_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        encoder.write_image(
            enhanced.as_raw(),
            WIDTH as u32,
            HEIGHT as u32,
            image::ExtendedColorType::L8,
        )?;

        Ok(png_data)
    }

    fn encode_png(raw_data: &[u8]) -> Result<Vec<u8>> {
        let raw_u8: Vec<u8> = raw_data
            .chunks_exact(2)
            .map(|chunk| u8::from_le_bytes([chunk[1]]))
            .collect();

        let mut processed = vec![0u8; (REMARKABLE_WIDTH * REMARKABLE_HEIGHT) as usize];

        for y in 0..REMARKABLE_HEIGHT {
            for x in 0..REMARKABLE_WIDTH {
                let src_idx =
                    (REMARKABLE_HEIGHT - 1 - y) + (REMARKABLE_WIDTH - 1 - x) * REMARKABLE_HEIGHT;
                let dst_idx = y * REMARKABLE_WIDTH + x;
                processed[dst_idx as usize] = Self::apply_curves(raw_u8[src_idx as usize]);
            }
        }

        let img = GrayImage::from_raw(REMARKABLE_WIDTH as u32, REMARKABLE_HEIGHT as u32, processed)
            .ok_or_else(|| anyhow::anyhow!("无法从原始数据创建图像"))?;

        let mut png_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        encoder.write_image(
            img.as_raw(),
            REMARKABLE_WIDTH as u32,
            REMARKABLE_HEIGHT as u32,
            image::ExtendedColorType::L8,
        )?;

        Ok(png_data)
    }

    fn apply_curves(value: u8) -> u8 {
        let normalized = value as f32 / 255.0;
        let adjusted = if normalized < 0.045 {
            0.0
        } else if normalized < 0.06 {
            (normalized - 0.045) / (0.06 - 0.045)
        } else {
            1.0
        };
        (adjusted * 255.0) as u8
    }

    pub fn save_image(&self, filename: &str) -> Result<()> {
        let mut png_file = File::create(filename)?;
        png_file.write_all(&self.data)?;
        info!("PNG图像已保存到 {}", filename);
        Ok(())
    }

    pub fn base64(&self) -> Result<String> {
        let base64_image = general_purpose::STANDARD.encode(&self.data);
        Ok(base64_image)
    }

    pub fn get_image_data(&self) -> Result<Vec<u8>> {
        Ok(self.data.clone())
    }
}
