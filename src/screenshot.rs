use anyhow::{anyhow, Result};
use image::GrayImage;
use std::fs::File;
use std::io::Write;
use base64::engine::general_purpose;

const REMARKABLE_WIDTH: u32 = 768;
const REMARKABLE_HEIGHT: u32 = 1024;

pub struct Screenshot {
    data: Vec<u8>,
}

impl Screenshot {
    pub fn new() -> Result<Self> {
        // 由于没有 xochitl 进程和 fb0，我们返回一个空白图像
        let data = Self::create_blank_image()?;
        Ok(Self { data })
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
        println!("PNG image saved to {}", filename);
        Ok(())
    }

    pub fn base64(&self) -> Result<String> {
        let base64_image = general_purpose::STANDARD.encode(&self.data);
        Ok(base64_image)
    }
}
