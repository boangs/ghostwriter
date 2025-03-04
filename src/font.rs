use freetype::{Library, Face};
use anyhow::Result;
use std::rc::Rc;
use crate::util::Asset;

pub struct FontRenderer {
    face: Face,
}

impl FontRenderer {
    pub fn new() -> Result<Self> {
        let lib = Library::init()?;
        let font_data = Asset::get("LXGWWenKaiGBScreen.ttf")
            .ok_or_else(|| anyhow::anyhow!("无法找到字体文件 LXGWWenKaiGBScreen.ttf"))?
            .data;
        
        let font_data = Rc::new(font_data.to_vec());
        let face = lib.new_memory_face(font_data, 0)
            .map_err(|e| anyhow::anyhow!("加载字体失败: {}", e))?;
        
        Ok(FontRenderer { face })
    }

    pub fn get_char_strokes(&self, c: char, size: f32) -> Result<(Vec<Vec<(i32, i32)>>, i32)> {
        self.face.set_pixel_sizes(0, size as u32)?;
        self.face.load_char(
            c as usize, 
            freetype::face::LoadFlag::RENDER | freetype::face::LoadFlag::MONOCHROME
        )?;
        
        let glyph = self.face.glyph();
        let bitmap = glyph.bitmap();
        let width = bitmap.width() as usize;
        let height = bitmap.rows() as usize;
        let buffer = bitmap.buffer();
        
        let mut strokes = Vec::new();
        let mut current_stroke = Vec::new();
        let scale = 1.0;
        
        // 获取字形的基线偏移
        let metrics = glyph.metrics();
        let baseline_offset = -(metrics.horiBearingY >> 6) as i32;  // 转换为像素
        
        for y in 0..height {
            let mut in_stroke = false;
            for x in 0..width {
                let byte = buffer[y * bitmap.pitch() as usize + (x >> 3)];
                let bit = (byte >> (7 - (x & 7))) & 1;
                
                if bit == 1 {
                    if !in_stroke {
                        // 开始新的笔画
                        if !current_stroke.is_empty() {
                            strokes.push(current_stroke);
                            current_stroke = Vec::new();
                        }
                        in_stroke = true;
                    }
                    let px = (x as f32 * scale) as i32;
                    let py = (y as f32 * scale) as i32;
                    current_stroke.push((px, py));
                } else if in_stroke {
                    in_stroke = false;
                }
            }
        }
        
        if !current_stroke.is_empty() {
            strokes.push(current_stroke);
        }
        
        Ok((strokes, baseline_offset))
    }

    pub fn char_to_svg(&self, c: char, size: f32, x: i32, y: i32) -> Result<String> {
        self.face.set_pixel_sizes(0, (size * 2.0) as u32)?;
        self.face.load_char(
            c as usize, 
            freetype::face::LoadFlag::DEFAULT
        )?;
        
        let glyph = self.face.glyph();
        let outline = glyph.outline().ok_or_else(|| anyhow::anyhow!("无法获取字符轮廓"))?;
        
        let points = outline.points();
        let contours = outline.contours();
        
        let scale = 0.02;
        let mut path_data = String::new();
        let mut start: usize = 0;
        
        for end in contours.iter() {
            let end_idx = *end as usize;
            
            path_data.push_str(&format!("M {} {} ", 
                x + (points[start].x as f32 * scale) as i32,
                y - (points[start].y as f32 * scale) as i32
            ));
            
            for i in (start + 1)..=end_idx {
                let point = points[i];
                path_data.push_str(&format!("L {} {} ",
                    x + (point.x as f32 * scale) as i32,
                    y - (point.y as f32 * scale) as i32
                ));
            }
            
            path_data.push('Z');
            start = end_idx + 1;
        }

        Ok(format!(
            r#"<path d="{}" fill="black" />"#,
            path_data
        ))
    }
}

#[allow(dead_code)]
fn optimize_strokes(strokes: Vec<Vec<(i32, i32)>>) -> Vec<Vec<(i32, i32)>> {
    let mut optimized: Vec<Vec<(i32, i32)>> = Vec::new();
    let mut current_stroke: Vec<(i32, i32)> = Vec::new();
    
    for stroke in strokes {
        if current_stroke.is_empty() {
            current_stroke = stroke;
            continue;
        }
        
        let last_point = *current_stroke.last().unwrap();
        let first_point = stroke[0];
        
        if (last_point.1 - first_point.1).abs() <= 1 {
            current_stroke.extend(stroke);
        } else {
            optimized.push(current_stroke);
            current_stroke = stroke;
        }
    }
    
    if !current_stroke.is_empty() {
        optimized.push(current_stroke);
    }
    
    optimized
} 