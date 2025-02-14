use anyhow::{Result, anyhow};
use image::GrayImage;
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom};
use base64::engine::general_purpose;
use std::path::Path;
use std::os::unix::io::AsRawFd;
use nix::ioctl_read;

// 这些值需要根据实际设备进行调整
const REMARKABLE_WIDTH: u32 = 1404;   // 请确认实际分辨率
const REMARKABLE_HEIGHT: u32 = 1872;  // 请确认实际分辨率
const DRM_DEVICE: &str = "/dev/dri/card0";
const DRM_CONNECTOR: &str = "LVDS-1";

// DRM ioctl 命令定义
ioctl_read!(drm_get_version, b'd', 0x00, drm_version);

#[repr(C)]
struct drm_version {
    version_major: i32,
    version_minor: i32,
    version_patchlevel: i32,
    name_len: usize,
    name: *mut u8,
    date_len: usize,
    date: *mut u8,
    desc_len: usize,
    desc: *mut u8,
}

pub struct Screenshot {
    data: Vec<u8>,
}

impl Screenshot {
    pub fn new() -> Result<Self> {
        if Path::new(DRM_DEVICE).exists() {
            println!("Found DRM device at {}", DRM_DEVICE);
            match Self::capture_from_drm() {
                Ok(data) => {
                    println!("Successfully captured from DRM device");
                    Ok(Self { data })
                }
                Err(e) => {
                    println!("Failed to capture from DRM: {}", e);
                    println!("Falling back to blank image");
                    let data = Self::create_blank_image()?;
                    Ok(Self { data })
                }
            }
        } else {
            println!("No DRM device found, using blank image");
            let data = Self::create_blank_image()?;
            Ok(Self { data })
        }
    }

    fn capture_from_drm() -> Result<Vec<u8>> {
        let file = File::open(DRM_DEVICE)?;
        let fd = file.as_raw_fd();

        // 获取 DRM 版本信息
        let mut version: drm_version = unsafe { std::mem::zeroed() };
        unsafe {
            drm_get_version(fd, &mut version)?;
        }

        println!("DRM version: {}.{}.{}", 
            version.version_major,
            version.version_minor,
            version.version_patchlevel
        );

        // 检查 LVDS 连接器状态
        let connector_path = format!("/sys/class/drm/card0-{}/status", DRM_CONNECTOR);
        if let Ok(status) = std::fs::read_to_string(&connector_path) {
            println!("LVDS connector status: {}", status.trim());
        }

        // TODO: 实现实际的帧缓冲区捕获
        // 1. 获取当前显示模式
        // 2. 映射帧缓冲区内存
        // 3. 读取像素数据
        // 4. 转换为 PNG 格式

        println!("DRM capture not fully implemented yet, returning blank image");
        Self::create_blank_image()
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
