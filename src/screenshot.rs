use anyhow::Result;
use image::GrayImage;
use log::{info, error};
use std::fs::File;
use std::io::{Write, Read, Seek};
use std::path::Path;
use std::process::Command;
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
        // 查找 xochitl 进程
        let output = Command::new("pidof")
            .arg("xochitl")
            .output()?;
        
        let stdout = String::from_utf8(output.stdout)?;
        let pid = stdout
            .trim()
            .split_whitespace()
            .next()
            .ok_or_else(|| anyhow::anyhow!("未找到 xochitl 进程"))?;
        
        info!("找到 xochitl 进程 PID: {}", pid);

        // 查找内存映射区域
        let maps_file = format!("/proc/{}/maps", pid);
        let maps_content = std::fs::read_to_string(&maps_file)?;
        
        // 查找包含 /dev/dri/card0 的内存区域
        let mem_region = maps_content
            .lines()
            .find(|line| line.contains("/dev/dri/card0"))
            .ok_or_else(|| anyhow::anyhow!("未找到显示内存区域"))?;

        info!("找到内存区域: {}", mem_region);

        // 解析内存区域地址
        let addr_range = mem_region
            .split_whitespace()
            .next()
            .ok_or_else(|| anyhow::anyhow!("无法解析内存地址"))?;

        let (start_addr, _) = addr_range
            .split_once('-')
            .ok_or_else(|| anyhow::anyhow!("无法解析地址范围"))?;

        let start_addr = u64::from_str_radix(start_addr, 16)?;
        info!("内存起始地址: 0x{:x}", start_addr);

        // 打开进程内存
        let mut mem_file = File::open(format!("/proc/{}/mem", pid))?;
        
        // 读取显示内存数据
        let mut buffer = vec![0u8; WINDOW_BYTES];
        mem_file.seek(std::io::SeekFrom::Start(start_addr))?;
        mem_file.read_exact(&mut buffer)?;
        
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
