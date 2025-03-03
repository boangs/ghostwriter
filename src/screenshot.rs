use anyhow::Result;
use image::GrayImage;
use log::{info, error};
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom};
use std::os::unix::io::AsRawFd;
use crate::constants::{REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use std::mem::size_of;

use base64::{engine::general_purpose, Engine as _};
use image::ImageEncoder;

const WIDTH: usize = 1872;
const HEIGHT: usize = 1404;
const BYTES_PER_PIXEL: usize = 2;
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
        // 打开物理内存设备
        let file = File::open("/dev/mem")?;
        info!("成功打开物理内存设备");

        let mut last_error = None;

        // 尝试每个可能的内存地址
        for &addr in DISPLAY_MEM_ADDRS.iter() {
            info!("尝试内存地址: 0x{:x}", addr);
            
            // 映射内存
            let result = unsafe {
                let ptr = libc::mmap(
                    std::ptr::null_mut(),
                    DISPLAY_MEM_SIZE,
                    libc::PROT_READ,
                    libc::MAP_SHARED,
                    file.as_raw_fd(),
                    addr as i64,
                );

                if ptr == libc::MAP_FAILED {
                    let err = std::io::Error::last_os_error();
                    error!("映射地址 0x{:x} 失败: {}", addr, err);
                    last_error = Some(err);
                    continue;
                }

                // 复制内存数据
                let slice = std::slice::from_raw_parts(ptr as *const u8, DISPLAY_MEM_SIZE);
                let mut buffer = vec![0u8; DISPLAY_MEM_SIZE];
                buffer.copy_from_slice(slice);

                // 取消映射
                libc::munmap(ptr, DISPLAY_MEM_SIZE);

                Ok(buffer)
            };

            // 如果这个地址成功了，就处理数据并返回
            if let Ok(buffer) = result {
                info!("成功从地址 0x{:x} 读取显示内存，大小: {} 字节", addr, buffer.len());
                return Self::process_image(buffer);
            }
        }

        // 如果所有地址都失败了，返回最后一个错误
        Err(anyhow::anyhow!("无法访问显示内存: {}", last_error.unwrap()))
    }

    fn process_image(data: Vec<u8>) -> Result<Vec<u8>> {
        // 将原始数据编码为PNG
        let png_data = Self::encode_png(&data)?;

        // 将PNG调整为指定大小
        let img = image::load_from_memory(&png_data)?;
        let resized_img = img.resize(
            OUTPUT_WIDTH,
            OUTPUT_HEIGHT,
            image::imageops::FilterType::Lanczos3,
        );

        // 将调整后的图像重新编码为PNG
        let mut resized_png_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut resized_png_data);
        encoder.write_image(
            resized_img.as_luma8().unwrap().as_raw(),
            OUTPUT_WIDTH,
            OUTPUT_HEIGHT,
            image::ExtendedColorType::L8,
        )?;

        Ok(resized_png_data)
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
