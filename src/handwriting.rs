use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::fs;
use std::path::PathBuf;
use crate::pen::Pen;
use crate::screenshot::Screenshot;
use crate::segmenter::analyze_image;
use std::time::Duration;
use std::thread::sleep;

pub struct HandwritingInput {
    pen: Arc<Mutex<Pen>>,
    strokes: Vec<Vec<(i32, i32)>>,
    is_writing: bool,
    temp_dir: PathBuf,
}

impl HandwritingInput {
    pub fn new(no_draw: bool) -> Result<Self> {
        // 创建临时目录
        let temp_dir = std::env::temp_dir().join("ghostwriter");
        fs::create_dir_all(&temp_dir)?;
        
        Ok(Self {
            pen: Arc::new(Mutex::new(Pen::new(no_draw))),
            strokes: Vec::new(),
            is_writing: false,
            temp_dir,
        })
    }

    pub fn start_stroke(&mut self, x: i32, y: i32) -> Result<()> {
        self.is_writing = true;
        let mut current_stroke = Vec::new();
        current_stroke.push((x, y));
        self.strokes.push(current_stroke);
        
        let mut pen = self.pen.lock().unwrap();
        pen.pen_down()?;
        pen.goto_xy((x, y))?;
        Ok(())
    }

    pub fn continue_stroke(&mut self, x: i32, y: i32) -> Result<()> {
        if self.is_writing {
            if let Some(current_stroke) = self.strokes.last_mut() {
                current_stroke.push((x, y));
                let mut pen = self.pen.lock().unwrap();
                pen.goto_xy((x, y))?;
                sleep(Duration::from_millis(1));
            }
        }
        Ok(())
    }

    pub fn end_stroke(&mut self) -> Result<()> {
        self.is_writing = false;
        let mut pen = self.pen.lock().unwrap();
        pen.pen_up()?;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.strokes.clear();
        self.is_writing = false;
    }

    pub fn capture_and_recognize(&self) -> Result<String> {
        // 1. 截取当前屏幕
        let screenshot = Screenshot::new()?;
        let img_data = screenshot.get_image_data()?;
        
        // 2. 保存图像到临时文件
        let temp_image = self.temp_dir.join("capture.png");
        fs::write(&temp_image, &img_data)?;
        
        // 3. 分析图像区域
        let regions = analyze_image(temp_image.to_str().unwrap())
            .map_err(|e| anyhow::anyhow!("图像分析失败: {}", e))?;
        
        // 4. 将区域信息添加到提示中
        let prompt = format!(
            "以下是手写内容的区域信息，请识别出文字内容：\n{}",
            regions
        );
        
        // TODO: 调用 OCR 服务识别文字
        // 这里需要添加实际的 OCR 调用
        
        Ok(prompt)
    }
} 