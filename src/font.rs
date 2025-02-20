use freetype::{Library, Face, Vector};
use anyhow::Result;
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
        let face = lib.new_memory_face(font_data, 0)?;
        face.set_pixel_sizes(0, 100)?; // 设置字体大小
        
        Ok(FontRenderer { face })
    }

    pub fn get_char_strokes(&self, c: char) -> Result<Vec<Vec<(i32, i32)>>> {
        self.face.load_char(c as usize, freetype::face::LoadFlag::NO_SCALE)?;
        
        let mut strokes = Vec::new();
        let outline = self.face.glyph().outline().unwrap();
        
        let mut current_stroke = Vec::new();
        let mut move_to = |x: f32, y: f32| {
            if !current_stroke.is_empty() {
                strokes.push(current_stroke.clone());
                current_stroke.clear();
            }
            current_stroke.push((x as i32, y as i32));
            Ok(())
        };
        
        let mut line_to = |x: f32, y: f32| {
            current_stroke.push((x as i32, y as i32));
            Ok(())
        };
        
        let mut conic_to = |x1: f32, y1: f32, x: f32, y: f32| {
            // 二次贝塞尔曲线
            let steps = 10;
            let x0 = current_stroke.last().unwrap().0 as f32;
            let y0 = current_stroke.last().unwrap().1 as f32;
            
            for i in 1..=steps {
                let t = i as f32 / steps as f32;
                let px = (1.0 - t).powi(2) * x0 + 2.0 * (1.0 - t) * t * x1 + t.powi(2) * x;
                let py = (1.0 - t).powi(2) * y0 + 2.0 * (1.0 - t) * t * y1 + t.powi(2) * y;
                current_stroke.push((px as i32, py as i32));
            }
            Ok(())
        };
        
        let mut cubic_to = |x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32| {
            // 三次贝塞尔曲线
            let steps = 10;
            let x0 = current_stroke.last().unwrap().0 as f32;
            let y0 = current_stroke.last().unwrap().1 as f32;
            
            for i in 1..=steps {
                let t = i as f32 / steps as f32;
                let px = (1.0 - t).powi(3) * x0 + 3.0 * t * (1.0 - t).powi(2) * x1 
                    + 3.0 * t.powi(2) * (1.0 - t) * x2 + t.powi(3) * x;
                let py = (1.0 - t).powi(3) * y0 + 3.0 * t * (1.0 - t).powi(2) * y1 
                    + 3.0 * t.powi(2) * (1.0 - t) * y2 + t.powi(3) * y;
                current_stroke.push((px as i32, py as i32));
            }
            Ok(())
        };
        
        outline.decompose(
            &mut move_to,
            &mut line_to,
            &mut conic_to,
            &mut cubic_to,
        )?;
        
        if !current_stroke.is_empty() {
            strokes.push(current_stroke);
        }
        
        Ok(strokes)
    }
} 