use anyhow::Result;
use rusttype::{Font, Scale, Point};
use std::thread::sleep;
use std::time::Duration;

const REMARKABLE_WIDTH: u32 = 768;
const REMARKABLE_HEIGHT: u32 = 1024;

pub struct Pen;

impl Pen {
    pub fn new(_no_draw: bool) -> Self {
        Self
    }

    pub fn draw_text(&mut self, text: &str, position: (i32, i32), size: f32) -> Result<()> {
        let font_data = include_bytes!("../assets/WenQuanYiMicroHei.ttf");
        let font = Font::try_from_bytes(font_data).unwrap();
        
        let scale = Scale::uniform(size);
        let v_metrics = font.v_metrics(scale);
        let glyphs: Vec<_> = font.layout(text, scale, Point { 
            x: position.0 as f32, 
            y: position.1 as f32 + v_metrics.ascent 
        }).collect();
        
        for glyph in glyphs {
            if let Some(outline) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    if v > 0.1 {
                        let x = outline.min.x as i32 + x as i32;
                        let y = outline.min.y as i32 + y as i32;
                        println!("Draw pixel at ({}, {})", x, y);
                    }
                });
            }
        }
        
        Ok(())
    }

    pub fn draw_bitmap(&mut self, bitmap: &Vec<Vec<bool>>) -> Result<()> {
        for (y, row) in bitmap.iter().enumerate() {
            for (x, &pixel) in row.iter().enumerate() {
                if pixel {
                    println!("Draw pixel at ({}, {})", x, y);
                }
            }
            sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}
