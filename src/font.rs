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
        face.set_pixel_sizes(0, 100)?;
        
        Ok(FontRenderer { face })
    }

    pub fn get_char_strokes(&self, c: char, size: f32) -> Result<Vec<Vec<(i32, i32)>>> {
        self.face.set_pixel_sizes(0, size as u32)?;
        self.face.load_char(c as usize, freetype::face::LoadFlag::NO_SCALE)?;
        
        let mut strokes = Vec::new();
        let outline = self.face.glyph().outline().unwrap();
        
        // 获取轮廓点
        let points: Vec<_> = outline.points().iter()
            .map(|p| (p.x as i32, p.y as i32))
            .collect();
            
        // 根据轮廓标记分割笔画
        let mut current_stroke = Vec::new();
        let contours = outline.contours();
        
        let mut point_index = 0;
        for &end_index in contours.iter() {
            while point_index <= end_index as usize {
                let point = points[point_index];
                current_stroke.push(point);
                
                if point_index == end_index as usize {
                    // 闭合轮廓
                    if !current_stroke.is_empty() {
                        current_stroke.push(current_stroke[0]);
                        strokes.push(current_stroke.clone());
                        current_stroke.clear();
                    }
                }
                
                point_index += 1;
            }
        }
        
        // 对每个笔画进行平滑处理
        let smoothed_strokes = strokes.into_iter()
            .map(|stroke| smooth_stroke(stroke))
            .collect();
        
        Ok(smoothed_strokes)
    }
}

// 平滑笔画路径
fn smooth_stroke(stroke: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    if stroke.len() < 3 {
        return stroke;
    }
    
    let mut result = Vec::new();
    let steps = 10;
    
    for i in 0..stroke.len() - 1 {
        let p0 = stroke[i];
        let p1 = stroke[i + 1];
        
        // 在两点之间插入平滑过渡点
        for step in 0..steps {
            let t = step as f32 / steps as f32;
            let x = p0.0 as f32 + (p1.0 - p0.0) as f32 * t;
            let y = p0.1 as f32 + (p1.1 - p0.1) as f32 * t;
            result.push((x as i32, y as i32));
        }
    }
    
    // 添加最后一个点
    result.push(*stroke.last().unwrap());
    result
} 