use rusttype::{point, Font, Scale, Point, Vector};
use anyhow::Result;
use crate::util::Asset;

pub struct FontRenderer {
    font_data: Vec<u8>,          // 存储字体数据
    font: Font<'static>,
}

impl FontRenderer {
    pub fn new() -> Result<Self> {
        // 加载字体数据
        let font_data = Asset::get("LXGWWenKaiScreen-Regular.ttf")
            .expect("Failed to load font")
            .data
            .to_vec();
            
        // 使用 Box::leak 将数据转换为 'static 生命周期
        let font_bytes = Box::leak(font_data.clone().into_boxed_slice());
        let font = Font::try_from_bytes(font_bytes)
            .expect("Failed to parse font");
            
        Ok(FontRenderer { 
            font_data: font_data,  // 保存数据所有权
            font 
        })
    }

    pub fn get_char_strokes(&self, c: char, size: f32) -> Vec<Vec<(i32, i32)>> {
        let scale = Scale::uniform(size);
        let glyph = self.font.glyph(c).scaled(scale);
        
        // 获取字形轮廓
        if let Some(outline) = glyph.exact_outline() {
            let mut strokes = Vec::new();
            let mut current_stroke = Vec::new();
            
            // 遍历轮廓的控制点
            for curve in outline.curves() {
                match curve {
                    rusttype::Curve::Line(p) => {
                        // 直线段
                        let point = (p.x as i32, p.y as i32);
                        current_stroke.push(point);
                    },
                    rusttype::Curve::Quadratic(c, p) => {
                        // 二次贝塞尔曲线，将其分解为多个点
                        let steps = 10;  // 曲线细分程度
                        for i in 0..=steps {
                            let t = i as f32 / steps as f32;
                            let x = (1.0 - t).powi(2) * current_stroke.last().unwrap().0 as f32
                                + 2.0 * (1.0 - t) * t * c.x
                                + t.powi(2) * p.x;
                            let y = (1.0 - t).powi(2) * current_stroke.last().unwrap().1 as f32
                                + 2.0 * (1.0 - t) * t * c.y
                                + t.powi(2) * p.y;
                            current_stroke.push((x as i32, y as i32));
                        }
                    },
                    rusttype::Curve::Cubic(c1, c2, p) => {
                        // 三次贝塞尔曲线，将其分解为多个点
                        let steps = 15;  // 曲线细分程度
                        for i in 0..=steps {
                            let t = i as f32 / steps as f32;
                            let x = (1.0 - t).powi(3) * current_stroke.last().unwrap().0 as f32
                                + 3.0 * (1.0 - t).powi(2) * t * c1.x
                                + 3.0 * (1.0 - t) * t.powi(2) * c2.x
                                + t.powi(3) * p.x;
                            let y = (1.0 - t).powi(3) * current_stroke.last().unwrap().1 as f32
                                + 3.0 * (1.0 - t).powi(2) * t * c1.y
                                + 3.0 * (1.0 - t) * t.powi(2) * c2.y
                                + t.powi(3) * p.y;
                            current_stroke.push((x as i32, y as i32));
                        }
                    }
                }
            }
            
            if !current_stroke.is_empty() {
                strokes.push(current_stroke);
            }
            
            strokes
        } else {
            Vec::new()
        }
    }
}

fn manhattan_distance(p1: (i32, i32), p2: (i32, i32)) -> i32 {
    (p1.0 - p2.0).abs() + (p1.1 - p2.1).abs()
} 