use freetype::{Library, Face, Vector};
use anyhow::Result;
use std::rc::Rc;
use crate::util::Asset;

pub struct FontRenderer {
    face: Face,
}

impl FontRenderer {
    pub fn new() -> Result<Self> {
        let lib = Library::init()?;
        let font_data = Asset::get("LXGWWenKaiScreen-Regular.ttf")
            .expect("Failed to load font")
            .data;
        
        let font_data = Rc::new(font_data.to_vec());
        let face = lib.new_memory_face(font_data, 0)?;
        
        Ok(FontRenderer { face })
    }

    pub fn get_char_strokes(&self, c: char, size: f32) -> Result<Vec<Vec<(i32, i32)>>> {
        // 设置字体大小
        self.face.set_pixel_sizes(0, size as u32)?;
        
        // 加载字符并进行光栅化
        self.face.load_char(
            c as usize, 
            freetype::face::LoadFlag::RENDER | freetype::face::LoadFlag::MONOCHROME
        )?;
        
        let bitmap = self.face.glyph().bitmap();
        let metrics = self.face.glyph().metrics();
        
        // 获取字形位图数据
        let mut strokes = Vec::new();
        let mut current_stroke = Vec::new();
        
        let buffer = bitmap.buffer();
        let width = bitmap.width() as usize;
        let height = bitmap.rows() as usize;
        
        // 遍历位图的每一行
        for y in 0..height {
            let mut in_stroke = false;
            let mut stroke_start = 0;
            
            // 遍历每一行的每个像素
            for x in 0..width {
                let byte = buffer[y * bitmap.pitch() as usize + (x >> 3)];
                let bit = (byte >> (7 - (x & 7))) & 1;
                
                if bit == 1 && !in_stroke {
                    // 开始新的笔画
                    in_stroke = true;
                    stroke_start = x;
                } else if (bit == 0 || x == width - 1) && in_stroke {
                    // 结束当前笔画
                    in_stroke = false;
                    let stroke_end = if bit == 1 { x } else { x - 1 };
                    
                    // 添加水平线段
                    let mut stroke = Vec::new();
                    for px in (stroke_start..=stroke_end).step_by(1) {
                        stroke.push((
                            px as i32,
                            y as i32
                        ));
                    }
                    if !stroke.is_empty() {
                        strokes.push(stroke);
                    }
                }
            }
        }
        
        // 对笔画进行优化，合并相邻的水平线段
        let optimized_strokes = optimize_strokes(strokes);
        
        Ok(optimized_strokes)
    }
}

fn optimize_strokes(strokes: Vec<Vec<(i32, i32)>>) -> Vec<Vec<(i32, i32)>> {
    let mut optimized = Vec::new();
    let mut current_stroke = Vec::new();
    
    for stroke in strokes {
        if current_stroke.is_empty() {
            current_stroke = stroke;
            continue;
        }
        
        // 检查是否可以与前一个笔画合并
        let last_point = *current_stroke.last().unwrap();
        let first_point = stroke[0];
        
        if (last_point.1 - first_point.1).abs() <= 1 {
            // 可以合并
            current_stroke.extend(stroke);
        } else {
            // 不能合并，开始新的笔画
            optimized.push(current_stroke);
            current_stroke = stroke;
        }
    }
    
    if !current_stroke.is_empty() {
        optimized.push(current_stroke);
    }
    
    optimized
} 