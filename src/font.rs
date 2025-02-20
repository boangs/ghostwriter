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