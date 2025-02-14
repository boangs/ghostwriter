use anyhow::{Result, anyhow};
use image::{GrayImage, ImageBuffer, Luma};
use image::ImageEncoder;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use base64::engine::general_purpose;
use base64::Engine;
use std::io::Read;

const REMARKABLE_WIDTH: u32 = 1404;
const REMARKABLE_HEIGHT: u32 = 1872;
const FPGA_DEVICE: &str = "/dev/i2c-2";  // FPGA 通过 I2C 总线连接
const FPGA_ADDR: u8 = 0x22;              // FPGA 的 I2C 地址
const FRAMEBUFFER_SIZE: usize = (REMARKABLE_WIDTH * REMARKABLE_HEIGHT) as usize;

pub struct Screenshot {
    data: Vec<u8>,
}

impl Screenshot {
    pub fn new() -> Result<Self> {
        if Path::new("/sys/bus/i2c/devices/2-0022/name").exists() {
            let name = std::fs::read_to_string("/sys/bus/i2c/devices/2-0022/name")?;
            if name.trim() == "cumulus-bridge" {
                println!("Found cumulus-bridge FPGA");
                
                if Path::new(FPGA_DEVICE).exists() {
                    println!("Found I2C device for FPGA communication");
                    
                    // 尝试从 FPGA 读取帧缓冲区
                    match Self::capture_from_fpga() {
                        Ok(raw_data) => {
                            let data = Self::process_raw_data(&raw_data)?;
                            return Ok(Self { data });
                        }
                        Err(e) => {
                            println!("Failed to capture from FPGA: {}", e);
                        }
                    }
                }
                
                println!("Falling back to blank image");
                let data = Self::create_blank_image()?;
                Ok(Self { data })
            } else {
                Err(anyhow!("Unexpected FPGA device: {}", name.trim()))
            }
        } else {
            Err(anyhow!("FPGA device not found"))
        }
    }

    fn capture_from_fpga() -> Result<Vec<u8>> {
        let mut file = File::open(FPGA_DEVICE)?;
        let mut raw_data = vec![0u8; FRAMEBUFFER_SIZE];
        
        // 发送读取帧缓冲区的命令
        file.write_all(&[0x00])?; // 假设 0x00 是读取命令
        
        // 读取帧缓冲区数据
        file.read_exact(&mut raw_data)?;
        
        Ok(raw_data)
    }

    fn process_raw_data(raw_data: &[u8]) -> Result<Vec<u8>> {
        // 创建灰度图像
        let img: ImageBuffer<Luma<u8>, Vec<u8>> = ImageBuffer::from_raw(
            REMARKABLE_WIDTH,
            REMARKABLE_HEIGHT,
            raw_data.to_vec(),
        ).ok_or_else(|| anyhow!("Failed to create image from raw data"))?;

        // 转换为 PNG
        let mut png_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        encoder.write_image(
            img.as_raw(),
            REMARKABLE_WIDTH,
            REMARKABLE_HEIGHT,
            image::ExtendedColorType::L8,
        )?;

        Ok(png_data)
    }

    fn create_blank_image() -> Result<Vec<u8>> {
        println!("Creating blank image with dimensions {}x{}", REMARKABLE_WIDTH, REMARKABLE_HEIGHT);
        let img = GrayImage::new(REMARKABLE_WIDTH, REMARKABLE_HEIGHT);
        let mut png_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        encoder.write_image(
            img.as_raw(),
            REMARKABLE_WIDTH,
            REMARKABLE_HEIGHT,
            image::ExtendedColorType::L8,
        )?;
        Ok(png_data)
    }

    pub fn save_image(&self, filename: &str) -> Result<()> {
        let mut png_file = File::create(filename)?;
        png_file.write_all(&self.data)?;
        println!("Image saved to {}", filename);
        Ok(())
    }

    pub fn base64(&self) -> Result<String> {
        let base64_image = general_purpose::STANDARD.encode(&self.data);
        Ok(base64_image)
    }
}

impl Drop for Screenshot {
    fn drop(&mut self) {
        // 清理资源
        println!("Cleaning up Screenshot resources");
    }
}

