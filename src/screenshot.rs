use anyhow::Result;
use image::GrayImage;
use log::{info, error};
use std::fs::File;
use std::io::{Write, Read};
use std::os::unix::io::AsRawFd;
use crate::constants::{REMARKABLE_WIDTH, REMARKABLE_HEIGHT};

use base64::{engine::general_purpose, Engine as _};
use image::ImageEncoder;

const WIDTH: usize = 1872;
const HEIGHT: usize = 1404;
const BYTES_PER_PIXEL: usize = 2;
const WINDOW_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;

const OUTPUT_WIDTH: u32 = 768;
const OUTPUT_HEIGHT: u32 = 1024;

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
        // 直接打开显示设备
        let file = File::open("/dev/dri/card0")?;
        info!("成功打开显示设备");

        // 使用 mmap 映射设备内存
        let buffer = unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                WINDOW_BYTES,
                libc::PROT_READ,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            );

            if ptr == libc::MAP_FAILED {
                return Err(anyhow::anyhow!(
                    "mmap 失败: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let slice = std::slice::from_raw_parts(ptr as *const u8, WINDOW_BYTES);
            let mut buffer = vec![0u8; WINDOW_BYTES];
            buffer.copy_from_slice(slice);

            // 取消映射
            libc::munmap(ptr, WINDOW_BYTES);

            buffer
        };

        info!("读取显示内存成功，大小: {} 字节", buffer.len());

        // 处理图像数据
        let processed_data = Self::process_image(buffer)?;

        Ok(processed_data)
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
