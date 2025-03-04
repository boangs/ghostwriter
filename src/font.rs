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
        let width = metrics.width >> 6;
        let height = metrics.height >> 6;
        
        // 使用字形的实际大小计算缩放比例
        let scale = size / height as f32;
        
        // 计算坐标范围
        let max_coord = (size * 1.2) as i32;  // 给一些余量
        
        println!("字形尺寸: {}x{}, 缩放比例: {}", width, height, scale);
        
        // 计算基准偏移，使字形居中
        let bearing_x = (metrics.horiBearingX >> 6) as i32;
        let bearing_y = (metrics.horiBearingY >> 6) as i32;
        
        // 将FreeType的轮廓点转换为我们的Point结构
        let mut contours_points = Vec::new();
        let mut contour_start = 0;
        
        for (_i, &end_idx) in contours.iter().enumerate() {
            let end_idx = end_idx as usize;
            let mut contour = Vec::new();
            
            // 处理当前轮廓的所有点
            let mut i = contour_start;
            while i <= end_idx {
                let p = points[i];
                let tag = tags[i];
                
                // 转换坐标，保持原始方向
                let x = ((p.x as f32 * scale) as i32).min(max_coord).max(-max_coord);
                let y = ((p.y as f32 * scale) as i32).min(max_coord).max(-max_coord);
                let point = Point::new(x, y);
                
                if (tag & 0x01) != 0 {  // on-curve point
                    contour.push(point);
                } else {  // off-curve point (control point)
                    // 获取下一个点
                    let next_i = if i == end_idx { contour_start } else { i + 1 };
                    let next_p = points[next_i];
                    
                    let next_x = ((next_p.x as f32 * scale) as i32).min(max_coord).max(-max_coord);
                    let next_y = ((next_p.y as f32 * scale) as i32).min(max_coord).max(-max_coord);
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
            
            contours_points.push(contour);
            contour_start = end_idx + 1;
        }
        
        // 创建笔画提取器
        let mut extractor = StrokeExtractor::new();
        
        // 对每个轮廓分别处理
        for contour in &contours_points {
            // 检测角点
            extractor.detect_corners_for_contour(contour);
        }
        
        println!("检测到的角点数量: {}", extractor.corners.len());
        
        // 提取笔画
        extractor.extract_strokes_from_contours(&contours_points);
        println!("提取的笔画数量: {}", extractor.strokes.len());
        
        // 转换笔画格式并进行平移
        let strokes = extractor.strokes.iter()
            .map(|stroke| {
                stroke.iter()
                    .map(|p| {
                        // 在最终输出时进行坐标系转换
                        let screen_x = p.x + bearing_x;
                        let screen_y = -p.y + bearing_y; // 翻转y轴并应用偏移
                        (screen_x, screen_y)
                    })
                    .collect()
            })
            .collect();
        
        Ok((strokes, bearing_y))
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
    
    fn detect_corners_for_contour(&mut self, contour: &[Point]) {
        const MIN_CORNER_ANGLE: f32 = std::f32::consts::PI * 0.15;
        const SAMPLE_DISTANCE: usize = 3;
        
        let n = contour.len();
        if n < SAMPLE_DISTANCE * 2 {
            return;
        }
        
        // 计算轮廓的方向（顺时针还是逆时针）
        let mut area = 0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += contour[i].x * contour[j].y - contour[j].x * contour[i].y;
        }
        let is_clockwise = area < 0;
        
        for i in 0..n {
            let p0 = &contour[i];
            let p1 = &contour[(i + SAMPLE_DISTANCE) % n];
            let p2 = &contour[(i + n - SAMPLE_DISTANCE) % n];
            
            let tangent1 = Point::new(p1.x - p0.x, p1.y - p0.y);
            let tangent2 = Point::new(p2.x - p0.x, p2.y - p0.y);
            
            let len1 = (tangent1.x * tangent1.x + tangent1.y * tangent1.y) as f32;
            let len2 = (tangent2.x * tangent2.x + tangent2.y * tangent2.y) as f32;
            
            if len1 < 4.0 || len2 < 4.0 {
                continue;
            }
            
            let corner = Corner::new(*p0, tangent1, tangent2);
            
            // 根据轮廓方向调整角点判断
            let angle = if is_clockwise {
                2.0 * std::f32::consts::PI - corner.angle
            } else {
                corner.angle
            };
            
            if angle > MIN_CORNER_ANGLE && angle < std::f32::consts::PI * 1.85 {
                self.corners.push(Corner {
                    point: *p0,
                    angle,
                    tangent1,
                    tangent2,
                });
            }
        }
    }
    
    fn extract_strokes_from_contours(&mut self, contours: &[Vec<Point>]) {
        if self.corners.len() < 2 {
            // 如果角点太少，使用轮廓作为笔画
            for contour in contours {
                if contour.len() > 2 {
                    self.strokes.push(contour.clone());
                }
            }
            return;
        }

        // 对每个轮廓分别处理
        for contour in contours {
            if contour.len() < 3 {
                continue;
            }

            // 计算这个轮廓的主要方向
            let mut dx_sum = 0;
            let mut dy_sum = 0;
            for i in 0..(contour.len() - 1) {
                dx_sum += contour[i + 1].x - contour[i].x;
                dy_sum += contour[i + 1].y - contour[i].y;
            }
            let main_direction = (dy_sum as f32).atan2(dx_sum as f32);

            // 找出属于这个轮廓的角点
            let mut contour_corners: Vec<&Corner> = self.corners.iter()
                .filter(|c| contour.contains(&c.point))
                .collect();

            // 如果轮廓上没有足够的角点，直接使用轮廓点
            if contour_corners.len() < 2 {
                self.strokes.push(contour.clone());
                continue;
            }

            // 根据主要方向对角点排序
            if main_direction.abs() < std::f32::consts::PI / 4.0 {
                // 水平方向优先
                contour_corners.sort_by_key(|c| c.point.x);
            } else {
                // 垂直方向优先
                contour_corners.sort_by_key(|c| -c.point.y);
            }

            // 创建笔画
            let mut current_stroke = Vec::new();
            let mut last_point = contour_corners[0].point;
            current_stroke.push(last_point);

            // 在角点之间寻找合适的路径点
            for corner in contour_corners.iter().skip(1) {
                let mut path_points = Vec::new();
                let mut found = false;

                // 在轮廓点中寻找连接路径
                for i in 0..contour.len() {
                    if contour[i] == last_point {
                        found = true;
                        path_points.push(contour[i]);
                        continue;
                    }
                    if found {
                        path_points.push(contour[i]);
                        if contour[i] == corner.point {
                            break;
                        }
                    }
                }

                // 如果没找到路径，使用直线连接
                if path_points.is_empty() {
                    let dist = last_point.distance(&corner.point);
                    let steps = (dist * 0.2) as usize + 1;
                    for t in 1..steps {
                        let t = t as f32 / steps as f32;
                        let x = last_point.x as f32 * (1.0 - t) + corner.point.x as f32 * t;
                        let y = last_point.y as f32 * (1.0 - t) + corner.point.y as f32 * t;
                        current_stroke.push(Point::new(x as i32, y as i32));
                    }
                } else {
                    // 使用找到的路径点
                    current_stroke.extend(path_points);
                }

                last_point = corner.point;
            }

            // 添加笔画
            if current_stroke.len() > 1 {
                println!("添加笔画: 长度={}, 起点=({},{}), 终点=({},{})",
                    current_stroke.len(),
                    current_stroke[0].x, current_stroke[0].y,
                    current_stroke.last().unwrap().x, current_stroke.last().unwrap().y
                );
                self.strokes.push(current_stroke);
            }
        }

        // 对所有笔画进行平滑处理
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