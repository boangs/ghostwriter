use freetype::{Library, Face};
use anyhow::Result;
use std::rc::Rc;
use crate::util::Asset;
use std::collections::HashMap;
use serde_json;

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

    pub fn get_char_strokes(&self, c: char, size: f32) -> Result<(Vec<Vec<(i32, i32)>>, i32, i32)> {
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
        let mut current_stroke = Vec::new();
        let scale = 1.0;
        
        // 获取字形的基线偏移和实际宽度
        let metrics = glyph.metrics();
        let baseline_offset = -(metrics.horiBearingY >> 6) as i32;  // 转换为像素
        let char_width = (metrics.horiAdvance >> 6) as i32;  // 转换为像素
        
        for y in 0..height {
            let mut in_stroke = false;
            for x in 0..width {
                let byte = buffer[y * bitmap.pitch() as usize + (x >> 3)];
                let bit = (byte >> (7 - (x & 7))) & 1;
                
                if bit == 1 {
                    if !in_stroke {
                        // 开始新的笔画
                        if !current_stroke.is_empty() {
                            strokes.push(current_stroke);
                            current_stroke = Vec::new();
                        }
                        in_stroke = true;
                    }
                    let px = (x as f32 * scale) as i32;
                    let py = (y as f32 * scale) as i32;
                    current_stroke.push((px, py));
                } else if in_stroke {
                    in_stroke = false;
                }
            }
        }
        
        if !current_stroke.is_empty() {
            strokes.push(current_stroke);
        }
        
        Ok((strokes, baseline_offset, char_width))
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

pub struct HersheyFont {
    glyphs: HashMap<char, Vec<Vec<(i32, i32)>>>,
    json_glyphs: HashMap<char, Vec<Vec<(f32, f32)>>>,
}

impl HersheyFont {
    pub fn new() -> Result<Self> {
        // 加载 Heiti.hf.txt 字体文件
        let font_data = Asset::get("Heiti.hf.txt")
            .ok_or_else(|| anyhow::anyhow!("无法找到字体文件 Heiti.hf.txt"))?
            .data;
        
        let font_str = String::from_utf8_lossy(&font_data);
        let mut glyphs = HashMap::new();
        
        // 解析字体文件
        for line in font_str.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() != 2 {
                continue;
            }
            
            let ch = parts[0].chars().next()
                .ok_or_else(|| anyhow::anyhow!("无效的字符"))?;
                
            let strokes: Vec<Vec<(i32, i32)>> = parts[1].split('M')
                .filter(|s| !s.is_empty())
                .map(|stroke| {
                    stroke.split('L')
                        .filter_map(|point| {
                            let coords: Vec<&str> = point.trim().split(',').collect();
                            if coords.len() == 2 {
                                Some((
                                    coords[0].parse::<i32>().unwrap_or(0),
                                    coords[1].parse::<i32>().unwrap_or(0)
                                ))
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .collect();
                
            glyphs.insert(ch, strokes);
        }
        
        // 加载 JSON 格式的笔画数据
        let json_data = Asset::get("strokes.json")
            .ok_or_else(|| anyhow::anyhow!("无法找到字体文件 strokes.json"))?
            .data;
            
        let json_str = String::from_utf8_lossy(&json_data);
        let json_map: serde_json::Value = serde_json::from_str(&json_str)?;
        let mut json_glyphs = HashMap::new();
        
        for (unicode, strokes) in json_map.as_object().unwrap() {
            // 解析 Unicode 码点
            let hex = unicode.trim_start_matches("U+");
            let code_point = u32::from_str_radix(hex, 16)?;
            let ch = char::from_u32(code_point)
                .ok_or_else(|| anyhow::anyhow!("无效的 Unicode 码点"))?;
                
            // 解析笔画数据
            let strokes = strokes.as_array()
                .ok_or_else(|| anyhow::anyhow!("无效的笔画数据"))?
                .iter()
                .map(|stroke| {
                    stroke.as_array()
                        .unwrap()
                        .iter()
                        .map(|point| {
                            let point = point.as_array().unwrap();
                            (
                                point[0].as_f64().unwrap() as f32,
                                point[1].as_f64().unwrap() as f32
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
                
            json_glyphs.insert(ch, strokes);
        }
        
        Ok(HersheyFont { glyphs, json_glyphs })
    }
    
    pub fn get_char_strokes(&self, c: char, size: f32) -> Result<(Vec<Vec<(i32, i32)>>, i32, i32)> {
        let strokes = self.glyphs.get(&c)
            .ok_or_else(|| anyhow::anyhow!("字符 {} 不在字体中", c))?;
            
        // 计算字符的边界框
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;
        
        for stroke in strokes {
            for &(x, y) in stroke {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
        
        // 计算缩放比例
        let scale = size / 1000.0;  // Hershey 字体通常基于 1000 单位网格
        
        // 缩放和转换笔画
        let scaled_strokes: Vec<Vec<(i32, i32)>> = strokes.iter()
            .map(|stroke| {
                stroke.iter()
                    .map(|&(x, y)| {
                        (
                            (x as f32 * scale) as i32,
                            (y as f32 * scale) as i32
                        )
                    })
                    .collect()
            })
            .collect();
            
        // 计算字符宽度和基线偏移
        let char_width = ((max_x - min_x) as f32 * scale) as i32;
        let baseline_offset = (max_y as f32 * scale) as i32;
        
        Ok((scaled_strokes, baseline_offset, char_width))
    }

    pub fn get_char_strokes_json(&self, c: char) -> Result<Vec<Vec<(f32, f32)>>> {
        self.json_glyphs.get(&c)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("字符 {} 不在 JSON 字体数据中", c))
    }
} 