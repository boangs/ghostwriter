use anyhow::Result;
use image::GrayImage;
use log::{info, error};
use std::fs::File;
use std::io::Write;
use std::process;
use crate::constants::{INPUT_WIDTH, INPUT_HEIGHT, REMARKABLE_WIDTH, REMARKABLE_HEIGHT};

use base64::{engine::general_purpose, Engine as _};
use image::ImageEncoder;
use drm::control::{Device as DrmDevice, ResourceHandle};
use drm::Device as _;

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
        // 打开 DRM 设备
        let card = File::open("/dev/dri/card0")?;
        info!("成功打开 DRM 设备");

        // 获取资源句柄
        let res_handles = card.resource_handles()?;
        info!("获取到 {} 个 CRTC", res_handles.crtcs.len());

        if res_handles.crtcs.is_empty() {
            return Err(anyhow::anyhow!("未找到可用的 CRTC"));
        }

        // 获取第一个 CRTC 的帧缓冲
        let crtc = res_handles.crtcs[0];
        let fb = card.get_framebuffer(crtc)?;
        info!("获取到帧缓冲，大小: {}x{}", fb.width, fb.height);

        // 读取帧缓冲数据
        let mut buffer = vec![0u8; WINDOW_BYTES];
        let mut map_file = File::open("/dev/dri/card0")?;
        map_file.read_exact(&mut buffer)?;
        info!("读取帧缓冲数据成功，大小: {} 字节", buffer.len());

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
