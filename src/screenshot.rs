use anyhow::{Result, Context};
use image::{GrayImage, DynamicImage, ImageBuffer, Rgba};
use log::{info, error, warn};
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom, BufRead, BufReader};
use std::process::Command;
use std::path::PathBuf;
use crate::constants::{REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use std::mem::size_of;

use base64::{engine::general_purpose, Engine as _};
use image::ImageEncoder;

// 注意：这里宽高是反的，因为 remarkable 的屏幕是竖向的
const HEIGHT: usize = 2154;  // remarkable paper pro 的实际宽度
const WIDTH: usize = 1624;   // remarkable paper pro 的实际高度
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
        let screenshot_data = Self::take_screenshot().context("截取屏幕失败")?;
        Ok(Screenshot {
            data: screenshot_data,
        })
    }

    fn take_screenshot() -> Result<Vec<u8>> {
        // 获取 xochitl 进程的 PID
        let output = Command::new("pidof")
            .arg("xochitl")
            .output()
            .context("执行 pidof 命令失败")?;
        let pid = String::from_utf8(output.stdout)
            .context("解析 PID 失败")?
            .trim()
            .to_string();
        info!("找到 xochitl 进程 PID: {}", pid);

        // 读取内存映射
        let maps_path = format!("/proc/{}/maps", pid);
        let maps_file = File::open(&maps_path)
            .context(format!("打开 {} 失败", maps_path))?;
        let reader = BufReader::new(maps_file);
        let mut lines: Vec<String> = reader.lines()
            .collect::<std::io::Result<_>>()
            .context("读取内存映射失败")?;
        lines.reverse();

        // 查找 /dev/dri/card0 相关的内存区域
        let mut memory_range = None;
        for (i, line) in lines.iter().enumerate() {
            if line.contains("/dev/dri/card0") {
                info!("找到 /dev/dri/card0 行: {}", line);
                if i + 1 < lines.len() {
                    let next_line = &lines[i + 1];
                    info!("下一行内容: {}", next_line);
                    let range = next_line.split_whitespace().next()
                        .ok_or_else(|| anyhow::anyhow!("无法解析内存范围"))?;
                    memory_range = Some(range.to_string());
                    break;
                }
            }
        }

        let range = memory_range.ok_or_else(|| anyhow::anyhow!("未找到显示内存区域"))?;
        let (start_str, end_str) = range.split_once('-')
            .ok_or_else(|| anyhow::anyhow!("无法解析内存范围"))?;
        let start = u64::from_str_radix(start_str, 16)
            .context("解析起始地址失败")?;
        let end = u64::from_str_radix(end_str, 16)
            .context("解析结束地址失败")?;
        
        info!("找到内存区域: {} (start: 0x{:x}, end: 0x{:x})", range, start, end);

        // 使用 dd 命令读取内存
        let temp_file = PathBuf::from("/tmp/remarkable_screen.raw");
        let dd_output = Command::new("dd")
            .arg(format!("if=/proc/{}/mem", pid))
            .arg(format!("of={}", temp_file.display()))
            .arg(format!("bs=1024"))
            .arg(format!("skip={}", start / 1024))  // 转换为1024字节块
            .arg(format!("count={}", (end - start) / 1024))
            .output()
            .context("执行 dd 命令失败")?;

        if !dd_output.status.success() {
            let error = String::from_utf8_lossy(&dd_output.stderr);
            error!("dd 命令执行失败: {}", error);
            return Err(anyhow::anyhow!("dd 命令执行失败: {}", error));
        }

        info!("dd 命令执行成功，读取临时文件");

        // 读取临时文件
        let mut raw_data = Vec::new();
        File::open(&temp_file)
            .context("打开临时文件失败")?
            .read_to_end(&mut raw_data)
            .context("读取临时文件失败")?;

        // 删除临时文件
        if let Err(e) = std::fs::remove_file(&temp_file) {
            warn!("删除临时文件失败: {}", e);
        }

        // 检查数据大小
        if raw_data.len() < WINDOW_BYTES {
            return Err(anyhow::anyhow!(
                "读取的数据大小不足，期望 {} 字节，实际 {} 字节",
                WINDOW_BYTES,
                raw_data.len()
            ));
        }

        // 只取需要的部分
        let buffer = raw_data[..WINDOW_BYTES].to_vec();

        // 处理图像数据
        let processed_data = Self::process_image(buffer)
            .context("处理图像数据失败")?;

        Ok(processed_data)
    }

    fn process_image(data: Vec<u8>) -> Result<Vec<u8>> {
        // 创建 RGBA 图像
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(WIDTH as u32, HEIGHT as u32, data.clone())
            .ok_or_else(|| anyhow::anyhow!("无法从原始数据创建图像"))?;

        // 转换为灰度图
        let mut gray_img = GrayImage::new(WIDTH as u32, HEIGHT as u32);
        for (x, y, pixel) in img.enumerate_pixels() {
            let gray_value = ((pixel[0] as f32 * 0.299 + 
                             pixel[1] as f32 * 0.587 + 
                             pixel[2] as f32 * 0.114) * 1.2) as u8;
            gray_img.put_pixel(x, y, image::Luma([gray_value]));
        }

        // 增强对比度
        let mut enhanced = gray_img.clone();
        for pixel in enhanced.pixels_mut() {
            let value = pixel[0];
            if value < 128 {
                pixel[0] = value.saturating_mul(2);
            } else {
                pixel[0] = value.saturating_add((255 - value) / 2);
            }
        }
        
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
