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
        self.face.load_char(c as usize, freetype::face::LoadFlag::RENDER | freetype::face::LoadFlag::MONOCHROME)?;
        
        let bitmap = self.face.glyph().bitmap();
        let metrics = self.face.glyph().metrics();
        
        let mut strokes: Vec<Vec<(i32, i32)>> = Vec::new();
        let mut current_stroke: Vec<(i32, i32)> = Vec::new();
        
        let buffer = bitmap.buffer();
        let width = bitmap.width() as usize;
        let height = bitmap.rows() as usize;
        
        for y in 0..height {
            let mut in_stroke = false;
            let mut stroke_start = 0;
            
            for x in 0..width {
                let byte = buffer[y * bitmap.pitch() as usize + (x >> 3)];
                let bit = (byte >> (7 - (x & 7))) & 1;
                
                if bit == 1 && !in_stroke {
                    in_stroke = true;
                    stroke_start = x;
                } else if (bit == 0 || x == width - 1) && in_stroke {
                    in_stroke = false;
                    let stroke_end = if bit == 1 { x } else { x - 1 };
                    
                    let mut stroke: Vec<(i32, i32)> = Vec::new();
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
        
        let optimized_strokes = optimize_strokes(strokes);
        
        Ok(optimized_strokes)
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