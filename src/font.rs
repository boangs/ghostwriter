use rusttype::{point, Font, Scale};
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
        let glyph = glyph.positioned(point(0.0, 0.0));

        if let Some(bitmap) = glyph.pixel_bounding_box() {
            let mut strokes = Vec::new();
            let mut current_stroke = Vec::new();
            let mut last_point = None;

            glyph.draw(|x, y, v| {
                if v > 0.5 {
                    let point = (
                        x as i32 + bitmap.min.x,
                        y as i32 + bitmap.min.y
                    );

                    // 如果与上一个点不连续，开始新的笔画
                    if let Some(last) = last_point {
                        if !is_connected(last, point) {
                            if !current_stroke.is_empty() {
                                strokes.push(current_stroke.clone());
                                current_stroke.clear();
                            }
                        }
                    }

                    current_stroke.push(point);
                    last_point = Some(point);
                }
            });

            if !current_stroke.is_empty() {
                strokes.push(current_stroke);
            }

            strokes
        } else {
            Vec::new()
        }
    }
}

fn is_connected(p1: (i32, i32), p2: (i32, i32)) -> bool {
    let dx = (p1.0 - p2.0).abs();
    let dy = (p1.1 - p2.1).abs();
    dx <= 1 && dy <= 1
} 