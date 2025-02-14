use anyhow::{Result, anyhow};
use image::GrayImage;
use image::ImageEncoder;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use base64::engine::general_purpose;
use base64::Engine;

const REMARKABLE_WIDTH: u32 = 1404;
const REMARKABLE_HEIGHT: u32 = 1872;
const FPGA_DEVICE: &str = "/dev/i2c-2";  // FPGA 通过 I2C 总线连接
const FPGA_ADDR: u8 = 0x22;              // FPGA 的 I2C 地址

pub struct Screenshot {
    data: Vec<u8>,
}

impl Screenshot {
    pub fn new() -> Result<Self> {
        // 检查 FPGA 设备
        if Path::new("/sys/bus/i2c/devices/2-0022/name").exists() {
            let name = std::fs::read_to_string("/sys/bus/i2c/devices/2-0022/name")?;
            if name.trim() == "cumulus-bridge" {
                println!("Found cumulus-bridge FPGA");
                
                // 检查 I2C 设备是否可访问
                if Path::new(FPGA_DEVICE).exists() {
                    println!("Found I2C device for FPGA communication");
                    
                    // TODO: 实现通过 I2C 与 FPGA 通信
                    // 1. 打开 I2C 设备
                    // 2. 发送适当的命令
                    // 3. 读取显示缓冲区数据
                    
                    println!("Note: Direct FPGA communication not yet implemented");
                }
                
                // 暂时返回空白图像
                let data = Self::create_blank_image()?;
                Ok(Self { data })
            } else {
                Err(anyhow!("Unexpected FPGA device: {}", name.trim()))
            }
        } else {
            Err(anyhow!("FPGA device not found"))
        }
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
        println!("Note: Currently saving blank image as FPGA communication is not implemented");
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

