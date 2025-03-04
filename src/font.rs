use freetype::{Library, Face};
use anyhow::Result;
use std::rc::Rc;
use crate::util::Asset;

pub struct FontRenderer {
    face: Face,
}

impl FontRenderer {
    pub fn new() -> Result<Self> {
        let lib = Library::init()?;
        let font_data = Asset::get("LXGWWenKaiGBScreen.ttf")
            .ok_or_else(|| anyhow::anyhow!("无法找到字体文件 LXGWWenKaiGBScreen.ttf"))?
            .data;
        
        let font_data = Rc::new(font_data.to_vec());
        let face = lib.new_memory_face(font_data, 0)
            .map_err(|e| anyhow::anyhow!("加载字体失败: {}", e))?;
        
        Ok(FontRenderer { face })
    }

    pub fn get_char_strokes(&self, c: char, size: f32) -> Result<(Vec<Vec<(i32, i32)>>, i32)> {
        self.face.set_pixel_sizes(0, size as u32)?;
        self.face.load_char(
            c as usize, 
            freetype::face::LoadFlag::RENDER | freetype::face::LoadFlag::MONOCHROME
        )?;
        
        let glyph = self.face.glyph();
        let bitmap = glyph.bitmap();
        let width = bitmap.width() as usize;
        let height = bitmap.rows() as usize;
        let buffer = bitmap.buffer();
        
        let mut strokes = Vec::new();
        let mut visited = vec![vec![false; width]; height];
        
        // 获取字形的基线偏移
        let metrics = glyph.metrics();
        let baseline_offset = -(metrics.horiBearingY >> 6) as i32;
        
        // 首先找到所有的端点（笔画的起点和终点）
        let mut endpoints = Vec::new();
        for y in 0..height {
            for x in 0..width {
                if !is_black_pixel(buffer, x, y, width) {
                    continue;
                }
                
                // 计算周围黑色像素的数量
                let mut black_neighbors = 0;
                for (dx, dy) in &[(0, 1), (1, 0), (0, -1), (-1, 0)] {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        if is_black_pixel(buffer, nx as usize, ny as usize, width) {
                            black_neighbors += 1;
                        }
                    }
                }
                
                // 端点只有一个相邻的黑色像素
                if black_neighbors == 1 {
                    endpoints.push((x, y));
                }
            }
        }
        
        // 从每个端点开始追踪笔画
        for &(start_x, start_y) in &endpoints {
            if visited[start_y][start_x] {
                continue;
            }
            
            let mut stroke = Vec::new();
            let mut current = (start_x, start_y);
            
            while !visited[current.1][current.0] {
                visited[current.1][current.0] = true;
                stroke.push((current.0 as i32, current.1 as i32));
                
                // 寻找最佳的下一个点（基于方向连续性）
                let mut best_next = None;
                let mut best_direction_diff = std::f32::MAX;
                let current_direction = if stroke.len() >= 2 {
                    let prev = stroke[stroke.len() - 2];
                    let curr = stroke[stroke.len() - 1];
                    ((curr.0 - prev.0) as f32).atan2((curr.1 - prev.1) as f32)
                } else {
                    0.0
                };
                
                // 检查8个方向
                for (dx, dy) in &[(0, 1), (1, 1), (1, 0), (1, -1), 
                                (0, -1), (-1, -1), (-1, 0), (-1, 1)] {
                    let nx = current.0 as i32 + dx;
                    let ny = current.1 as i32 + dy;
                    
                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nx = nx as usize;
                        let ny = ny as usize;
                        
                        if !visited[ny][nx] && is_black_pixel(buffer, nx, ny, width) {
                            let new_direction = (*dx as f32).atan2(*dy as f32);
                            let direction_diff = (new_direction - current_direction).abs();
                            let direction_diff = if direction_diff > std::f32::consts::PI {
                                2.0 * std::f32::consts::PI - direction_diff
                            } else {
                                direction_diff
                            };
                            
                            if direction_diff < best_direction_diff {
                                best_direction_diff = direction_diff;
                                best_next = Some((nx, ny));
                            }
                        }
                    }
                }
                
                if let Some(next) = best_next {
                    current = next;
                } else {
                    break;
                }
            }
            
            if stroke.len() > 2 {
                // 平滑笔画路径
                let smoothed = smooth_stroke(&stroke);
                strokes.push(smoothed);
            }
        }
        
        Ok((strokes, baseline_offset))
    }

    pub fn char_to_svg(&self, c: char, size: f32, x: i32, y: i32) -> Result<String> {
        self.face.set_pixel_sizes(0, (size * 2.0) as u32)?;
        self.face.load_char(
            c as usize, 
            freetype::face::LoadFlag::DEFAULT
        )?;
        
        let glyph = self.face.glyph();
        let outline = glyph.outline().ok_or_else(|| anyhow::anyhow!("无法获取字符轮廓"))?;
        
        let points = outline.points();
        let contours = outline.contours();
        
        let scale = 0.02;
        let mut path_data = String::new();
        let mut start: usize = 0;
        
        for end in contours.iter() {
            let end_idx = *end as usize;
            
            path_data.push_str(&format!("M {} {} ", 
                x + (points[start].x as f32 * scale) as i32,
                y - (points[start].y as f32 * scale) as i32
            ));
            
            for i in (start + 1)..=end_idx {
                let point = points[i];
                path_data.push_str(&format!("L {} {} ",
                    x + (point.x as f32 * scale) as i32,
                    y - (point.y as f32 * scale) as i32
                ));
            }
            
            path_data.push('Z');
            start = end_idx + 1;
        }

        Ok(format!(
            r#"<path d="{}" fill="black" />"#,
            path_data
        ))
    }
}

#[allow(dead_code)]
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

// 使用Douglas-Peucker算法简化笔画路径
fn simplify_stroke(points: &[(i32, i32)]) -> Vec<(i32, i32)> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    
    let epsilon = 2.0; // 简化阈值
    let mut result = Vec::new();
    let mut stack = vec![(0, points.len() - 1)];
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    
    while let Some((start, end)) = stack.pop() {
        let mut max_dist = 0.0;
        let mut max_idx = start;
        
        let line_start = points[start];
        let line_end = points[end];
        
        for i in start + 1..end {
            let dist = perpendicular_distance(points[i], line_start, line_end);
            if dist > max_dist {
                max_dist = dist;
                max_idx = i;
            }
        }
        
        if max_dist > epsilon {
            keep[max_idx] = true;
            stack.push((start, max_idx));
            stack.push((max_idx, end));
        }
    }
    
    for (i, &point) in points.iter().enumerate() {
        if keep[i] {
            result.push(point);
        }
    }
    
    result
}

// 计算点到线段的垂直距离
fn perpendicular_distance(point: (i32, i32), line_start: (i32, i32), line_end: (i32, i32)) -> f32 {
    let (x, y) = point;
    let (x1, y1) = line_start;
    let (x2, y2) = line_end;
    
    if x1 == x2 && y1 == y2 {
        return (((x - x1) * (x - x1) + (y - y1) * (y - y1)) as f32).sqrt();
    }
    
    let numerator = ((x2 - x1) * (y1 - y) - (x1 - x) * (y2 - y1)).abs() as f32;
    let denominator = (((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1)) as f32).sqrt();
    
    numerator / denominator
}

// 辅助函数：检查像素是否为黑色
fn is_black_pixel(buffer: &[u8], x: usize, y: usize, width: usize) -> bool {
    let byte = buffer[y * ((width + 7) / 8) + (x >> 3)];
    (byte >> (7 - (x & 7))) & 1 == 1
}

// 平滑笔画路径
fn smooth_stroke(points: &[(i32, i32)]) -> Vec<(i32, i32)> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    
    let mut smoothed = Vec::with_capacity(points.len());
    smoothed.push(points[0]);
    
    // 使用移动平均来平滑路径
    let window_size = 3;
    for i in 1..points.len() - 1 {
        let start = i.saturating_sub(window_size / 2);
        let end = (i + window_size / 2 + 1).min(points.len());
        let count = end - start;
        
        let sum_x: i32 = points[start..end].iter().map(|p| p.0).sum();
        let sum_y: i32 = points[start..end].iter().map(|p| p.1).sum();
        
        smoothed.push((
            sum_x / count as i32,
            sum_y / count as i32
        ));
    }
    
    smoothed.push(*points.last().unwrap());
    smoothed
} 