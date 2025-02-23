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
        self.face.set_pixel_sizes(0, size as u32)?;
        self.face.load_char(
            c as usize, 
            freetype::face::LoadFlag::DEFAULT
        )?;
        
        let glyph = self.face.glyph();
        let outline = glyph.outline().ok_or_else(|| anyhow::anyhow!("无法获取字符轮廓"))?;
        
        let points = outline.points();
        let tags = outline.tags();
        let contours = outline.contours();
        
        let mut strokes = Vec::new();
        let mut start: usize = 0;
        
        let scale = 0.02;
        
        for end in contours.iter() {
            let mut current_stroke = Vec::new();
            let end_idx = *end as usize;
            
            for i in start..=end_idx {
                let point = points[i];
                let tag = tags[i];
                
                if tag & 0x1 != 0 {
                    let x = (point.x as f32 * scale) as i32;
                    let y = -(point.y as f32 * scale) as i32;
                    current_stroke.push((x, y));
                }
            }
            
            if !current_stroke.is_empty() {
                if current_stroke[0] != *current_stroke.last().unwrap() {
                    current_stroke.push(current_stroke[0]);
                }
                strokes.push(current_stroke);
            }
            
            start = end_idx + 1;
        }
        
        Ok(strokes)
    }

    pub fn char_to_svg(&self, c: char, size: f32, x: i32, y: i32) -> Result<String> {
        self.face.set_pixel_sizes(0, (size * 2.0) as u32)?;
        self.face.load_char(
            c as usize, 
            freetype::face::LoadFlag::RENDER
        )?;
        
        let glyph = self.face.glyph();
        let bitmap = glyph.bitmap();
        let metrics = glyph.metrics();
        
        Ok(format!(
            r#"<text x="{}" y="{}" font-family="LXGWWenKaiScreen-Regular" font-size="{}" fill="black">{}</text>"#,
            x,
            y + (size as i32),
            size,
            c
        ))
    }
}

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