use rusttype::{point, Font, Scale, Point, Vector};
use anyhow::Result;
use crate::util::Asset;
use ab_glyph_rasterizer::Rasterizer;
use std::collections::VecDeque;

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

        if let Some(bbox) = glyph.pixel_bounding_box() {
            let mut strokes = Vec::new();
            let mut points = Vec::new();
            
            // 获取字形的像素点
            glyph.draw(|x, y, v| {
                if v > 0.5 {
                    points.push((x as i32 + bbox.min.x, y as i32 + bbox.min.y));
                }
            });

            // 使用连通区域算法将点转换为笔画
            let mut visited = std::collections::HashSet::new();
            for &start_point in &points {
                if visited.contains(&start_point) {
                    continue;
                }

                let mut stroke = Vec::new();
                let mut queue = VecDeque::new();
                queue.push_back(start_point);
                visited.insert(start_point);

                while let Some(point) = queue.pop_front() {
                    stroke.push(point);

                    // 检查8个方向的相邻点
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            
                            let next = (point.0 + dx, point.1 + dy);
                            if points.contains(&next) && !visited.contains(&next) {
                                queue.push_back(next);
                                visited.insert(next);
                            }
                        }
                    }
                }

                if stroke.len() > 1 {
                    // 对笔画点进行排序，使其更连续
                    optimize_stroke_path(&mut stroke);
                    strokes.push(stroke);
                }
            }

            strokes
        } else {
            Vec::new()
        }
    }
}

// 优化笔画路径，使点的连接更平滑
fn optimize_stroke_path(stroke: &mut Vec<(i32, i32)>) {
    if stroke.len() <= 2 {
        return;
    }

    let mut optimized = Vec::with_capacity(stroke.len());
    optimized.push(stroke[0]);
    
    let mut current = stroke[0];
    while optimized.len() < stroke.len() {
        let mut best_next = None;
        let mut min_dist = std::i32::MAX;
        
        for &point in stroke.iter() {
            if optimized.contains(&point) {
                continue;
            }
            
            let dist = manhattan_distance(current, point);
            if dist < min_dist {
                min_dist = dist;
                best_next = Some(point);
            }
        }
        
        if let Some(next) = best_next {
            optimized.push(next);
            current = next;
        } else {
            break;
        }
    }
    
    *stroke = optimized;
}

fn manhattan_distance(p1: (i32, i32), p2: (i32, i32)) -> i32 {
    (p1.0 - p2.0).abs() + (p1.1 - p2.1).abs()
} 