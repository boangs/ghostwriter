use anyhow::Result;
use image::{GrayImage, DynamicImage};
use log::{info, error};
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom};
use std::process::Command;
use crate::constants::{REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use base64::{Engine, engine::general_purpose};
use image::ImageEncoder;

pub struct Screenshot {
    width: u32,
    height: u32,
    data: Vec<u8>,  // 添加 data 字段存储图像数据
    last_content_y: i32,
}

impl Screenshot {
    pub fn new() -> Result<Self> {
        Ok(Self {
            width: 1624,  // remarkable 的实际宽度
            height: 2154, // remarkable 的实际高度
            data: Vec::new(),
            last_content_y: 50,  // 修改初始值为靠近顶部的位置
        })
    }

    pub fn get_image_data(&mut self) -> Result<Vec<u8>> {
        // 1. 获取 xochitl 进程 ID
        info!("开始获取 xochitl 进程 ID");
        let output = Command::new("pgrep")
            .arg("xochitl")
            .output()?;
            
        if !output.status.success() {
            error!("无法找到 xochitl 进程: {}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow::anyhow!("无法找到 xochitl 进程"));
        }
        
        let pid = String::from_utf8(output.stdout)?.trim().to_string();
        info!("找到 xochitl 进程 ID: {}", pid);
        
        // 2. 查找内存映射
        info!("开始读取内存映射文件");
        let maps_path = format!("/proc/{}/maps", pid);
        let maps = match std::fs::read_to_string(&maps_path) {
            Ok(content) => content,
            Err(e) => {
                error!("无法读取内存映射文件 {}: {}", maps_path, e);
                return Err(anyhow::anyhow!("无法读取内存映射文件"));
            }
        };
        
        info!("成功读取内存映射文件，开始查找显示内存区域");
        let mut memory_range = None;
        let lines: Vec<&str> = maps.lines().collect();
        
        for i in (0..lines.len()).rev() {
            if lines[i].contains("/dev/dri/card0") {
                info!("找到 DRI 设备映射: {}", lines[i]);
                if i + 1 < lines.len() {
                    memory_range = Some(lines[i + 1].split_whitespace().next().unwrap().to_string());
                    info!("找到相关内存区域: {}", memory_range.as_ref().unwrap());
                }
                break;
            }
        }
        
        let memory_range = memory_range.ok_or_else(|| {
            error!("在内存映射中未找到显示内存区域");
            anyhow::anyhow!("未找到显示内存区域")
        })?;
        
        let (start, _) = memory_range.split_once("-").unwrap();
        let start = u64::from_str_radix(start, 16)?;
        info!("显示内存起始地址: 0x{:x}", start);
        
        // 3. 查找实际图像数据的偏移量
        info!("开始查找图像数据偏移量");
        let mut mem_file = match std::fs::File::open(format!("/proc/{}/mem", pid)) {
            Ok(file) => file,
            Err(e) => {
                error!("无法打开进程内存文件: {}", e);
                return Err(anyhow::anyhow!("无法打开进程内存文件"));
            }
        };
        
        let mut offset: u64 = 0;
        let mut length: u64 = 2;
        let target_size = (self.width * self.height * 4) as u64;
        
        info!("目标图像大小: {} 字节", target_size);
        
        while length < target_size {
            offset += length - 2;
            if let Err(e) = mem_file.seek(SeekFrom::Start(start + offset + 8)) {
                error!("内存文件定位失败: {}", e);
                return Err(anyhow::anyhow!("内存文件定位失败"));
            }
            
            let mut header = [0u8; 8];
            if let Err(e) = mem_file.read_exact(&mut header) {
                error!("读取内存头部失败: {}", e);
                return Err(anyhow::anyhow!("读取内存头部失败"));
            }
            
            length = u64::from_le_bytes(header);
            info!("当前偏移量: 0x{:x}, 数据长度: {} 字节", offset, length);
        }
        
        // 4. 直接读取内存数据
        let skip = start + offset;
        let count = target_size;
        info!("最终读取参数: skip=0x{:x}, count={}", skip, count);
        
        // 直接从内存读取原始数据
        mem_file.seek(SeekFrom::Start(skip))?;
        let mut raw_data = vec![0u8; count as usize];
        mem_file.read_exact(&mut raw_data)?;
        
        // 直接将RGBA数据转换为灰度数据，跳过创建RGBA图像的步骤
        info!("将RGBA数据直接转换为灰度图");
        let mut gray_data = vec![0u8; (self.width * self.height) as usize];
        for i in 0..(self.width * self.height) as usize {
            let rgba = &raw_data[i * 4..(i + 1) * 4];
            // 使用标准的灰度转换公式，更准确地考虑人眼对不同颜色的敏感度
            // 公式: Gray = 0.299*R + 0.587*G + 0.114*B
            gray_data[i] = ((0.299 * rgba[0] as f32) + 
                           (0.587 * rgba[1] as f32) + 
                           (0.114 * rgba[2] as f32)) as u8;
        }
        
        // 直接创建灰度图
        let gray_img = image::GrayImage::from_raw(
            self.width,
            self.height,
            gray_data
        ).ok_or_else(|| anyhow::anyhow!("无法创建灰度图像"))?;
        
        info!("灰度图尺寸: {}x{}", gray_img.width(), gray_img.height());
        
        // 调整对比度
        let contrast_img = image::imageops::contrast(&gray_img, 2.0);
        
        // 在这里先分析内容位置
        let last_content_y = self.find_content_y_in_image(&contrast_img);
        info!("在原始尺寸图像中找到的内容位置: y = {}", last_content_y);
        
        // 然后再调整大小以优化存储
        let resized = image::imageops::resize(
            &contrast_img,
            self.width / 2,
            self.height / 2,
            image::imageops::FilterType::Lanczos3
        );
        info!("缩放后图像尺寸: {}x{}", resized.width(), resized.height());
        
        // 编码为高质量 PNG
        let mut final_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut final_data);
        encoder.write_image(
            resized.as_raw(),
            resized.width(),
            resized.height(),
            image::ColorType::L8.into()
        )?;
        
        // 保存最后找到的内容位置
        self.last_content_y = last_content_y;
        
        self.data = final_data.clone();
        Ok(final_data)
    }

    // 新增一个方法在原始大小的图像上查找内容位置
    fn find_content_y_in_image(&self, img: &GrayImage) -> i32 {
        let (width, height) = img.dimensions();
        info!("在原始尺寸图像中查找内容位置，图像尺寸: {}x{}", width, height);
        
        // 定义采样间隔和阈值
        let sample_interval = 10;  // 在原始大小的图像上可以用更大的间隔
        let min_dark_pixels = 4;   // 由于是原始大小，需要更多的暗像素才能确认是内容
        let dark_threshold = 200;  // 暗像素的阈值
        
        // 从底部向上扫描，找到第一个有内容的位置
        for y in (0..height).rev() {
            let mut dark_pixel_count = 0;
            
            // 在每一行采样检查
            for x in (0..width).step_by(sample_interval) {
                let pixel = img.get_pixel(x, y);
                if pixel[0] < dark_threshold {
                    dark_pixel_count += 1;
                    if dark_pixel_count >= min_dark_pixels {
                        info!("在原始图像中找到内容位置: y = {}", y);
                        return (y + 40) as i32;
                    }
                }
            }
        }
        
        info!("未找到内容，返回顶部位置");
        50  // 返回靠近顶部的位置，给第一行内容留出一些空间
    }

    pub fn save_image(&self, filename: &str) -> Result<()> {
        let mut png_file = File::create(filename)?;
        png_file.write_all(&self.data)?;
        info!("PNG图像已保存到 {}", filename);
        Ok(())
    }

    pub fn base64(&self) -> Result<String> {
        Ok(general_purpose::STANDARD.encode(&self.data))
    }

    pub fn find_last_content_y(&self) -> i32 {
        self.last_content_y  // 直接返回之前保存的位置
    }
}
