use rusttype::{point, Font, Scale};
use anyhow::Result;
use crate::util::Asset;

pub struct FontRenderer {
    font: Font<'static>,
}

impl FontRenderer {
    pub fn new() -> Result<Self> {
        // 加载字体并转换为 'static 生命周期的数据
        let font_data: Vec<u8> = Asset::get("LXGWWenKaiScreen-Regular.ttf")
            .expect("Failed to load font")
            .data
            .to_vec();  // 转换为拥有的数据
            
        let font = Font::try_from_bytes(&font_data)
            .expect("Failed to parse font");
            
        Ok(FontRenderer { font })
    }

    pub fn get_char_bitmap(&self, c: char, size: f32) -> Vec<(i32, i32)> {
        // 设置字体大小
        let scale = Scale::uniform(size);

        // 获取字形
        let glyph = self.font.glyph(c).scaled(scale);
        let glyph = glyph.positioned(point(0.0, 0.0));

        // 获取位图
        if let Some(bitmap) = glyph.pixel_bounding_box() {
            let mut points = Vec::new();
            
            // 遍历位图的每个像素
            glyph.draw(|x, y, v| {
                if v > 0.5 {
                    points.push((
                        x as i32 + bitmap.min.x,
                        y as i32 + bitmap.min.y
                    ));
                }
            });
            
            points
        } else {
            Vec::new()
        }
    }
} 