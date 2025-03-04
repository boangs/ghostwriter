use anyhow::Result;
use image::{GrayImage, DynamicImage, ImageBuffer, Luma};
use log::{info, error};
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom, BufRead, BufReader};
use std::os::unix::io::AsRawFd;
use std::process::Command;
use crate::constants::{REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use std::mem::size_of;

use base64::{engine::general_purpose, Engine as _};
use image::ImageEncoder;

const WIDTH: usize = 1624;  // 更新为正确的屏幕尺寸
const HEIGHT: usize = 2154;
const BYTES_PER_PIXEL: usize = 4;  // RGBA 格式
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
    width: u32,
    height: u32,
    data: Vec<u8>,  // 添加 data 字段存储图像数据
}

impl Screenshot {
    pub fn new() -> Result<Self> {
        Ok(Self {
            width: 1624,  // remarkable 的实际宽度
            height: 2154, // remarkable 的实际高度
            data: Vec::new(),
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
        
        // 4. 计算正确的读取参数
        let skip = start + offset;
        let count = target_size;
        info!("最终读取参数: skip=0x{:x}, count={}", skip, count);
        
        // 5. 使用 dd 获取原始数据，然后用 ffmpeg 转换
        let raw_file = "/tmp/ghostwriter/raw_capture.rgba";
        let png_file = "/tmp/ghostwriter/capture.png";
        
        // 首先用 dd 获取原始数据
        let dd_cmd = format!(
            "dd if=/proc/{}/mem of={} count={} bs=1024 iflag=skip_bytes,count_bytes skip={}",
            pid, raw_file, count, skip
        );
        
        info!("执行 dd 命令: {}", dd_cmd);
        let dd_output = Command::new("sh")
            .arg("-c")
            .arg(&dd_cmd)
            .output()?;
            
        if !dd_output.status.success() {
            error!("dd 命令执行失败: {}", String::from_utf8_lossy(&dd_output.stderr));
            return Err(anyhow::anyhow!("dd 命令执行失败"));
        }
        
        // 然后用 ffmpeg 转换为 PNG
        let ffmpeg_cmd = format!(
            "ffmpeg -f rawvideo -pixel_format rgba -video_size {}x{} -i {} -frames:v 1 {}",
            self.width, self.height, raw_file, png_file
        );
        
        info!("执行 ffmpeg 命令: {}", ffmpeg_cmd);
        let ffmpeg_output = Command::new("sh")
            .arg("-c")
            .arg(&ffmpeg_cmd)
            .output()?;
            
        if !ffmpeg_output.status.success() {
            error!("ffmpeg 命令执行失败: {}", String::from_utf8_lossy(&ffmpeg_output.stderr));
            return Err(anyhow::anyhow!("ffmpeg 命令执行失败"));
        }
        
        // 读取生成的 PNG 文件
        info!("读取生成的 PNG 文件");
        let png_data = match std::fs::read(png_file) {
            Ok(data) => {
                info!("成功读取 PNG 文件，大小: {} 字节", data.len());
                data
            }
            Err(e) => {
                error!("读取 PNG 文件失败: {}", e);
                return Err(anyhow::anyhow!("读取 PNG 文件失败"));
            }
        };
        
        // 使用 image crate 处理图像
        let img = image::load_from_memory(&png_data)?;
        
        // 1. 转换为灰度图
        let gray_img = img.into_luma8();
        
        // 2. 调整对比度
        let contrast_img = image::imageops::contrast(&gray_img, 2.0);
        
        // 3. 调整大小以优化识别
        let resized = image::imageops::resize(
            &contrast_img,
            self.width / 2,  // 降低分辨率以减小文件大小
            self.height / 2,
            image::imageops::FilterType::Lanczos3
        );
        
        // 4. 编码为高质量 PNG
        let mut final_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut final_data);
        encoder.write_image(
            resized.as_raw(),
            resized.width(),
            resized.height(),
            image::ColorType::L8
        )?;
        
        info!("图像处理完成，最终大小: {} 字节", final_data.len());
        self.data = final_data.clone();
        
        Ok(final_data)
    }

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
        let base64_image = base64::encode(&self.data);
        Ok(base64_image)
    }
}
