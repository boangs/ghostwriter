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
            freetype::face::LoadFlag::DEFAULT
        )?;
        
        let glyph = self.face.glyph();
        let outline = glyph.outline()
            .ok_or_else(|| anyhow::anyhow!("无法获取字符轮廓"))?;
        
        let points = outline.points();
        let tags = outline.tags();
        let contours = outline.contours();
        
        println!("字符 '{}' 的轮廓信息:", c);
        println!("点数量: {}", points.len());
        println!("轮廓数量: {}", contours.len());
        
        // 计算边界框和缩放比例
        let metrics = glyph.metrics();
        let units_per_em = self.face.units_per_em() as f32;
        let em_scale = size / units_per_em;
        
        // 计算字形的边界
        let width = (metrics.width >> 6) as f32;
        let height = (metrics.height >> 6) as f32;
        
        println!("字形尺寸: {}x{}, em比例: {}", width, height, em_scale);
        
        // 将FreeType的轮廓点转换为我们的Point结构
        let mut outline_points = Vec::new();
        let mut contour_start = 0;
        
        for (_i, &end_idx) in contours.iter().enumerate() {
            let end_idx = end_idx as usize;
            let mut contour = Vec::new();
            
            // 处理当前轮廓的所有点
            let mut i = contour_start;
            while i <= end_idx {
                let p = points[i];
                let tag = tags[i];
                
                // 转换坐标，应用em_scale并翻转y轴
                let x = (p.x as f32 * em_scale) as i32;
                let y = -(p.y as f32 * em_scale) as i32;  // 翻转y轴
                let point = Point::new(x, y);
                
                if (tag & 0x01) != 0 {  // on-curve point
                    contour.push(point);
                } else {  // off-curve point (control point)
                    // 获取下一个点
                    let next_i = if i == end_idx { contour_start } else { i + 1 };
                    let next_p = points[next_i];
                    
                    let next_x = (next_p.x as f32 * em_scale) as i32;
                    let next_y = -(next_p.y as f32 * em_scale) as i32;
                    let next_point = Point::new(next_x, next_y);
                    
                    // 在控制点之间插入中间点
                    let steps = 5;  // 减少插入点的数量
                    for t in 1..steps {
                        let t = t as f32 / steps as f32;
                        let mid_x = point.x as f32 * (1.0 - t) + next_point.x as f32 * t;
                        let mid_y = point.y as f32 * (1.0 - t) + next_point.y as f32 * t;
                        contour.push(Point::new(mid_x as i32, mid_y as i32));
                    }
                }
                i += 1;
            }
            
            // 确保轮廓闭合
            if !contour.is_empty() && contour[0] != *contour.last().unwrap() {
                contour.push(contour[0]);
            }
            
            outline_points.extend(contour);
            contour_start = end_idx + 1;
        }
        
        println!("提取的点数量: {}", outline_points.len());
        
        // 创建笔画提取器
        let mut extractor = StrokeExtractor::new();
        
        // 检测角点
        extractor.detect_corners(&outline_points);
        println!("检测到的角点数量: {}", extractor.corners.len());
        
        // 提取笔画
        extractor.extract_strokes();
        println!("提取的笔画数量: {}", extractor.strokes.len());
        
        // 获取基线偏移
        let baseline_offset = -(metrics.horiBearingY >> 6) as i32;
        
        // 转换笔画格式并进行平移
        let bearing_x = (metrics.horiBearingX >> 6) as i32;
        let strokes = extractor.strokes.iter()
            .map(|stroke| {
                stroke.iter()
                    .map(|p| (p.x + bearing_x, p.y + baseline_offset))
                    .collect()
            })
            .collect();
        
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        Point { x, y }
    }
    
    fn distance(&self, other: &Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        ((dx * dx + dy * dy) as f32).sqrt()
    }
    
    fn angle(&self, other: &Point) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dy as f32).atan2(dx as f32)
    }
}

#[derive(Debug)]
struct Corner {
    point: Point,
    angle: f32,      // 角点的转角
    tangent1: Point, // 前切向量
    tangent2: Point, // 后切向量
}

impl Corner {
    fn new(point: Point, tangent1: Point, tangent2: Point) -> Self {
        let angle = point.angle(&tangent2) - point.angle(&tangent1);
        let angle = if angle < 0.0 { angle + 2.0 * std::f32::consts::PI } else { angle };
        
        Corner {
            point,
            angle,
            tangent1,
            tangent2,
        }
    }
    
    // 判断是否是凹角（笔画交叉处的特征）
    fn is_concave(&self) -> bool {
        self.angle > std::f32::consts::PI
    }
}

struct StrokeExtractor {
    corners: Vec<Corner>,
    strokes: Vec<Vec<Point>>,
}

impl StrokeExtractor {
    fn new() -> Self {
        StrokeExtractor {
            corners: Vec::with_capacity(32), // 一般汉字的角点不会超过32个
            strokes: Vec::with_capacity(32), // 一般汉字的笔画不会超过32个
        }
    }
    
    fn detect_corners(&mut self, outline: &[Point]) {
        const MIN_CORNER_ANGLE: f32 = std::f32::consts::PI * 0.15; // 调整角点检测阈值
        const SAMPLE_DISTANCE: usize = 3; // 增加采样距离
        
        let n = outline.len();
        if n < SAMPLE_DISTANCE * 2 {
            return;
        }
        
        for i in 0..n {
            let p0 = &outline[i];
            let p1 = &outline[(i + SAMPLE_DISTANCE) % n];
            let p2 = &outline[(i + n - SAMPLE_DISTANCE) % n];
            
            let tangent1 = Point::new(p1.x - p0.x, p1.y - p0.y);
            let tangent2 = Point::new(p2.x - p0.x, p2.y - p0.y);
            
            // 计算切线长度
            let len1 = (tangent1.x * tangent1.x + tangent1.y * tangent1.y) as f32;
            let len2 = (tangent2.x * tangent2.x + tangent2.y * tangent2.y) as f32;
            
            // 忽略太短的切线
            if len1 < 4.0 || len2 < 4.0 {
                continue;
            }
            
            let corner = Corner::new(*p0, tangent1, tangent2);
            
            // 使用更严格的角点判断条件
            if corner.angle > MIN_CORNER_ANGLE && corner.angle < std::f32::consts::PI * 1.85 {
                self.corners.push(corner);
            }
        }
        
        // 合并距离太近的角点
        self.merge_close_corners();
    }
    
    fn merge_close_corners(&mut self) {
        let mut i = 0;
        while i < self.corners.len() {
            let mut j = i + 1;
            while j < self.corners.len() {
                if self.corners[i].point.distance(&self.corners[j].point) < 5.0 {
                    // 保留角度较大的角点
                    if self.corners[i].angle < self.corners[j].angle {
                        self.corners.swap(i, j);
                    }
                    self.corners.remove(j);
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
    
    fn extract_strokes(&mut self) {
        // 如果角点太少，直接使用轮廓点
        if self.corners.len() < 2 {
            return;
        }
        
        // 对角点按照x坐标排序
        self.corners.sort_by_key(|c| c.point.x);
        
        // 创建笔画
        let mut current_stroke = Vec::new();
        current_stroke.push(self.corners[0].point);
        
        for i in 1..self.corners.len() {
            let prev = &self.corners[i-1];
            let curr = &self.corners[i];
            
            let dist = prev.point.distance(&curr.point);
            let angle_diff = (prev.angle - curr.angle).abs();
            
            // 如果两个角点距离较近且角度差不大，认为是同一笔画
            if dist < 50.0 && angle_diff < std::f32::consts::PI * 0.5 {
                // 添加中间点
                let steps = (dist * 0.5) as usize + 1;
                for t in 1..steps {
                    let t = t as f32 / steps as f32;
                    let x = prev.point.x as f32 * (1.0 - t) + curr.point.x as f32 * t;
                    let y = prev.point.y as f32 * (1.0 - t) + curr.point.y as f32 * t;
                    current_stroke.push(Point::new(x as i32, y as i32));
                }
                current_stroke.push(curr.point);
            } else {
                // 结束当前笔画，开始新的笔画
                if current_stroke.len() > 1 {
                    println!("添加笔画: 长度={}, 起点=({},{}), 终点=({},{})",
                        current_stroke.len(),
                        current_stroke[0].x, current_stroke[0].y,
                        current_stroke.last().unwrap().x, current_stroke.last().unwrap().y
                    );
                    self.strokes.push(current_stroke);
                }
                current_stroke = Vec::new();
                current_stroke.push(curr.point);
            }
        }
        
        // 添加最后一个笔画
        if current_stroke.len() > 1 {
            println!("添加笔画: 长度={}, 起点=({},{}), 终点=({},{})",
                current_stroke.len(),
                current_stroke[0].x, current_stroke[0].y,
                current_stroke.last().unwrap().x, current_stroke.last().unwrap().y
            );
            self.strokes.push(current_stroke);
        }
        
        // 对笔画进行平滑处理
        for stroke in &mut self.strokes {
            let smoothed = smooth_stroke(&stroke.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>());
            *stroke = smoothed.into_iter().map(|(x, y)| Point::new(x, y)).collect();
        }
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