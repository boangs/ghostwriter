use anyhow::{Result, anyhow};
use image::GrayImage;
use std::fs::File;
use std::io::{Write, Read};
use base64::engine::general_purpose;
use std::path::Path;

// 这些值需要根据实际设备进行调整
const REMARKABLE_WIDTH: u32 = 1404;   // 请确认实际分辨率
const REMARKABLE_HEIGHT: u32 = 1872;  // 请确认实际分辨率
const LCDIF_PATH: &str = "/dev/mem";  // 内存映射设备
const LCDIF_ADDR: u64 = 0x32e00000;   // LCD 控制器的物理地址
const LCDIF_SIZE: usize = 0x10000;    // LCD 控制器的内存大小

pub struct Screenshot {
    data: Vec<u8>,
}

impl Screenshot {
    pub fn new() -> Result<Self> {
        // 检查 FPGA 桥接器
        if Path::new("/sys/bus/i2c/devices/2-0022").exists() {
            println!("Found rm-cumulus-bridge FPGA");
            
            // 尝试从 LCD 控制器读取数据
            match Self::read_from_lcdif() {
                Ok(data) => Ok(Self { data }),
                Err(e) => {
                    println!("Warning: Could not read from LCDIF: {}", e);
                    println!("Falling back to blank image");
                    let data = Self::create_blank_image()?;
                    Ok(Self { data })
                }
            }
        } else {
            Err(anyhow!("rm-cumulus-bridge FPGA not found"))
        }
    }

    fn read_from_lcdif() -> Result<Vec<u8>> {
        // 注意：这需要 root 权限
        let mut file = File::open(LCDIF_PATH)?;
        
        // 尝试读取 LCD 控制器内存
        let mut buffer = vec![0u8; LCDIF_SIZE];
        file.read_exact(&mut buffer)?;
        
        // TODO: 将原始数据转换为图像格式
        // 目前返回空白图像
        Self::create_blank_image()
    }

    fn create_blank_image() -> Result<Vec<u8>> {
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
