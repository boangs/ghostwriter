use anyhow::Result;
use image::{GrayImage, DynamicImage};
use log::{info, error};
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom};
use std::process::Command;
use crate::constants::{REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use base64::{Engine, engine::general_purpose};
use image::ImageEncoder;
use png;

#[allow(dead_code)]
const WIDTH: usize = 1624;  // 更新为正确的屏幕尺寸
#[allow(dead_code)]
const HEIGHT: usize = 2154;
#[allow(dead_code)]
const BYTES_PER_PIXEL: usize = 4;  // RGBA 格式
#[allow(dead_code)]
const WINDOW_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;

// reMarkable 显示内存的可能物理地址
#[allow(dead_code)]
const DISPLAY_MEM_ADDRS: [u64; 3] = [
    0x20000000,  // 第一个可能的地址
    0x9C000000,  // 第二个可能的地址
    0x10000000,  // 第三个可能的地址
];
#[allow(dead_code)]
const DISPLAY_MEM_SIZE: usize = WINDOW_BYTES;

#[allow(dead_code)]
const OUTPUT_WIDTH: u32 = 768;
#[allow(dead_code)]
const OUTPUT_HEIGHT: u32 = 1024;

// DRM ioctl 命令和结构体定义
#[allow(dead_code)]
const DRM_IOCTL_MODE_MAP_DUMB: u64 = 0xC01064B2;
#[allow(dead_code)]
const DRM_IOCTL_MODE_CREATE_DUMB: u64 = 0xC01064B0;

#[allow(dead_code)]
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

#[allow(dead_code)]
struct DrmModeCreateDumb {
    height: u32,
    width: u32,
    bpp: u32,
    flags: u32,
    handle: u32,
    pitch: u32,
    size: u64,
}

#[allow(dead_code)]
struct DrmModeMapDumb {
    handle: u32,
    pad: u32,
    offset: u64,
}

pub struct Screenshot {
    buffer: Vec<u8>,
    width: usize,
    height: usize,
}

impl Screenshot {
    pub fn new() -> Result<Self> {
        // 获取屏幕尺寸
        let width = 1404;
        let height = 1872;
        
        // 从设备读取屏幕数据
        let path = "/dev/fb0";
        let mut file = File::open(path)?;
        let mut buffer = vec![0u8; width * height / 8];
        std::io::Read::read_exact(&mut file, &mut buffer)?;
        
        Ok(Screenshot {
            buffer,
            width,
            height,
        })
    }
    
    pub fn get_image_data(&self) -> Result<Vec<u8>> {
        // 将原始缓冲区转换为PNG格式
        let mut png_data = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_data, self.width as u32, self.height as u32);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::One);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&self.buffer)?;
        }
        Ok(png_data)
    }
    
    pub fn find_last_content_y(&self) -> i32 {
        let mut last_content_y = 100; // 默认起始位置
        let scan_margin = 60; // 扫描边缘留白
        
        // 从底部向上扫描，寻找最后一行内容
        'outer: for y in (100..self.height - scan_margin).rev() {
            for x in scan_margin..self.width - scan_margin {
                if self.is_black_pixel(x, y) {
                    // 确认是否为有效内容（避免噪点）
                    if self.is_valid_content_line(y) {
                        last_content_y = y as i32;
                        info!("在y={}处发现内容", last_content_y);
                        break 'outer;
                    }
                }
            }
        }
        
        // 添加一些垂直间距
        last_content_y + 50
    }
    
    fn is_black_pixel(&self, x: usize, y: usize) -> bool {
        let byte_index = y * (self.width / 8) + (x / 8);
        let bit_index = 7 - (x % 8);
        
        if byte_index >= self.buffer.len() {
            return false;
        }
        
        (self.buffer[byte_index] & (1 << bit_index)) != 0
    }
    
    fn is_valid_content_line(&self, y: usize) -> bool {
        let min_black_pixels = 5; // 最小黑色像素数，用于过滤噪点
        let scan_margin = 60;
        let mut black_pixel_count = 0;
        
        // 在当前行扫描一定宽度范围内的黑色像素
        for x in scan_margin..self.width - scan_margin {
            if self.is_black_pixel(x, y) {
                black_pixel_count += 1;
                if black_pixel_count >= min_black_pixels {
                    return true;
                }
            }
        }
        
        false
    }

    #[allow(dead_code)]
    fn process_image(data: Vec<u8>) -> Result<Vec<u8>> {
        // 将 RGBA 数据转换为灰度图
        let mut gray_data = vec![0u8; WIDTH * HEIGHT];
        for i in 0..WIDTH * HEIGHT {
            let rgba = &data[i * 4..(i + 1) * 4];
            // 使用 RGB 平均值作为灰度值
            gray_data[i] = ((rgba[0] as u16 + rgba[1] as u16 + rgba[2] as u16) / 3) as u8;
        }

        let img = GrayImage::from_raw(WIDTH as u32, HEIGHT as u32, gray_data)
            .ok_or_else(|| anyhow::anyhow!("无法从原始数据创建图像"))?;

        // 将原始图像保存为调试用途
        let mut debug_png_data = Vec::new();
        let debug_encoder = image::codecs::png::PngEncoder::new(&mut debug_png_data);
        debug_encoder.write_image(
            img.as_raw(),
            WIDTH as u32,
            HEIGHT as u32,
            image::ExtendedColorType::L8,
        )?;
        
        // 保存原始图像到文件
        let mut debug_file = File::create("debug_original.png")?;
        debug_file.write_all(&debug_png_data)?;
        info!("保存原始图像到 debug_original.png");

        // 将 GrayImage 转换为 DynamicImage
        let dynamic_img = DynamicImage::ImageLuma8(img);

        // 调整图像大小
        let resized_img = dynamic_img.resize_exact(
            OUTPUT_WIDTH,
            OUTPUT_HEIGHT,
            image::imageops::FilterType::Lanczos3,
        );

        // 确保我们得到的是灰度图像
        let gray_img = resized_img.to_luma8();

        // 保存调整大小后的图像到文件
        let mut resized_debug_png_data = Vec::new();
        let resized_debug_encoder = image::codecs::png::PngEncoder::new(&mut resized_debug_png_data);
        resized_debug_encoder.write_image(
            gray_img.as_raw(),
            OUTPUT_WIDTH,
            OUTPUT_HEIGHT,
            image::ExtendedColorType::L8,
        )?;
        
        let mut resized_debug_file = File::create("debug_resized.png")?;
        resized_debug_file.write_all(&resized_debug_png_data)?;
        info!("保存调整大小后的图像到 debug_resized.png");

        // 编码为 PNG
        let mut png_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        encoder.write_image(
            gray_img.as_raw(),
            OUTPUT_WIDTH,
            OUTPUT_HEIGHT,
            image::ExtendedColorType::L8,
        )?;

        // 输出 base64 编码的图像数据前几个字符，用于调试
        let base64_image = general_purpose::STANDARD.encode(&png_data);
        info!("Base64 图像数据预览: {}...", &base64_image[..100]);

        Ok(png_data)
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
        png_file.write_all(&self.buffer)?;
        info!("PNG图像已保存到 {}", filename);
        Ok(())
    }

    pub fn base64(&self) -> Result<String> {
        Ok(general_purpose::STANDARD.encode(&self.buffer))
    }
}
