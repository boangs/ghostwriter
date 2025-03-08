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
    json_glyphs: HashMap<char, (Vec<(f32, f32)>, Vec<i32>)>, // (coords, pointTypes)
}

impl HersheyFont {
    pub fn new() -> Result<Self> {
        // 加载 JSON 格式的笔画数据
        let json_data = Asset::get("handstrokes.json")
            .ok_or_else(|| anyhow::anyhow!("无法找到字体文件 handstrokes.json"))?
            .data;
            
        let json_str = String::from_utf8_lossy(&json_data);
        let json_map: serde_json::Value = serde_json::from_str(&json_str)?;
        let mut json_glyphs = HashMap::new();
        
        if let Some(obj) = json_map.as_object() {
            for (char_str, data) in obj {
                // 解析 Unicode 字符
                let ch = if let Some(first_char) = char_str.chars().next() {
                    first_char
                } else {
                    continue;
                };
                
                // 解析坐标和点类型
                if let (Some(coords), Some(point_types)) = (
                    data.get("coord").and_then(|c| c.as_array()),
                    data.get("pointType").and_then(|p| p.as_array())
                ) {
                    let coords: Vec<(f32, f32)> = coords.chunks(2)
                        .filter_map(|chunk| {
                            if chunk.len() == 2 {
                                Some((
                                    chunk[0].as_f64()? as f32,
                                    chunk[1].as_f64()? as f32
                                ))
                            } else {
                                None
                            }
                        })
                        .collect();
                        
                    let point_types: Vec<i32> = point_types.iter()
                        .filter_map(|pt| pt.as_i64().map(|n| n as i32))
                        .collect();
                        
                    if coords.len() == point_types.len() {
                        json_glyphs.insert(ch, (coords, point_types));
                    }
                }
            }
        }
        
        Ok(HersheyFont { json_glyphs })
    }

    pub fn get_char_strokes(&self, c: char, size: f32) -> Result<(Vec<Vec<(i32, i32)>>, i32, i32)> {
        let (coords, point_types) = self.json_glyphs.get(&c)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("字符 {} 不在字体数据中", c))?;
        
        // 计算字符的边界框
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        
        for &(x, y) in &coords {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        
        // 计算原始尺寸
        let original_width = max_x - min_x;
        let original_height = max_y - min_y;
        
        // 计算基本缩放比例（以汉字为基准）
        let base_scale = size / original_height.max(original_width);
        
        // 判断是否为全角标点符号
        let is_fullwidth_punct = match c {
            '，' | '。' | '、' | '：' | '；' | '！' | '？' | '（' | '）' |
            '『' | '』' | '「' | '」' | '《' | '》' | '"' | '"' | ''' | ''' |
            '【' | '】' | '〈' | '〉' | '…' | '—' | '～' | '·' => true,
            _ => false
        };
        
        // 根据字符类型调整最终缩放比例
        let scale = if c.is_ascii_alphabetic() {
            base_scale * 0.6  // 英文字母缩小到汉字的 60%
        } else if c.is_ascii_punctuation() {
            base_scale * 0.2  // ASCII 标点符号缩小到汉字的 20%
        } else if is_fullwidth_punct {
            base_scale * 0.2  // 全角标点符号缩小到汉字的 20%
        } else {
            base_scale  // 汉字保持原始大小
        };
        
        // 将坐标点按笔画分组
        let mut strokes: Vec<Vec<(i32, i32)>> = Vec::new();
        let mut current_stroke = Vec::new();
        
        for (i, (&(x, y), &point_type)) in coords.iter().zip(point_types.iter()).enumerate() {
            if point_type == 0 || i == 0 {
                if !current_stroke.is_empty() {
                    strokes.push(current_stroke);
                }
                current_stroke = Vec::new();
            }
            
            // 坐标转换
            let px = ((x - min_x) * scale) as i32;
            let py = ((y - min_y) * scale) as i32;
            
            current_stroke.push((px, py));
        }
        
        if !current_stroke.is_empty() {
            strokes.push(current_stroke);
        }
        
        // 基线偏移
        let baseline_offset = 0;
        
        // 根据字符类型调整字符宽度
        let char_width = if c.is_ascii_alphabetic() {
            ((original_width * scale) as i32 + (size * 0.1) as i32).max(5)
        } else if c.is_ascii_punctuation() {
            ((original_width * scale) as i32 + (size * 0.05) as i32).max(3)
        } else if is_fullwidth_punct {
            ((original_width * scale) as i32 + (size * 0.1) as i32).max(4)
        } else {
            (original_width * scale) as i32 + (size * 0.2) as i32
        };
        
        Ok((strokes, baseline_offset, char_width))
    }
} 