use anyhow::Result;
use evdev::{Device, EventType, InputEvent};
use log::{debug, info, error};
use std::thread::sleep;
use std::time::Duration;
use freetype::Library;
use crate::util::Asset;

const INPUT_WIDTH: i32 = 15725;
const INPUT_HEIGHT: i32 = 20967;
const REMARKABLE_WIDTH: i32 = 1620;
const REMARKABLE_HEIGHT: i32 = 2160;

pub struct Pen {
    device: Option<Device>,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        Self {
            device: if no_draw { None } else { Some(Device::open("/dev/input/event2").unwrap()) },
        }
    }

    pub fn write_text(&mut self, text: &str) -> Result<()> {
        debug!("开始书写文本: {}", text);
        let start_x = 200;    // 起始位置右移一些
        let start_y = 200;    // 起始位置下移一些
        let char_width = 100;  // 增加字符间距
        let line_height = 150; // 增加行间距
        let mut current_x = start_x;
        let mut current_y = start_y;

        for c in text.chars() {
            // 如果到达行尾，换行
            if current_x > 700 {  // 减小行宽，避免超出屏幕
                current_x = start_x;
                current_y += line_height;
            }

            // 获取字符的笔画信息
            if let Ok(strokes) = self.get_char_strokes(c) {
                // 绘制每个笔画
                for stroke in strokes {
                    if stroke.len() < 2 {
                        continue;
                    }
                    
                    // 移动到笔画起点
                    self.pen_up()?;
                    let (x, y) = stroke[0];
                    self.goto_xy_screen((current_x + x, current_y + y))?;
                    self.pen_down()?;
                    
                    // 绘制笔画
                    for &(x, y) in stroke.iter().skip(1) {
                        self.goto_xy_screen((current_x + x, current_y + y))?;
                    }
                    
                    sleep(Duration::from_millis(50)); // 笔画间停顿
                }
            }
            
            current_x += char_width;
            sleep(Duration::from_millis(100)); // 字符间停顿
        }
        Ok(())
    }

    pub fn draw_line_screen(&mut self, p1: (i32, i32), p2: (i32, i32)) -> Result<()> {
        self.draw_line(screen_to_input(p1), screen_to_input(p2))
    }

    pub fn draw_line(&mut self, (x1, y1): (i32, i32), (x2, y2): (i32, i32)) -> Result<()> {
        let length = ((x2 as f32 - x1 as f32).powf(2.0) + (y2 as f32 - y1 as f32).powf(2.0)).sqrt();
        
        // 如果长度为0，说明是同一个点，直接返回
        if length == 0.0 {
            return Ok(());
        }
        
        // 5.0 是点之间的最大距离
        let steps = (length / 5.0).ceil() as i32;
        let dx = (x2 - x1) / steps;
        let dy = (y2 - y1) / steps;

        self.pen_up()?;
        self.goto_xy((x1, y1))?;
        self.pen_down()?;

        for i in 0..steps {
            let x = x1 + dx * i;
            let y = y1 + dy * i;
            self.goto_xy((x, y))?;
        }

        self.pen_up()?;
        Ok(())
    }

    pub fn draw_bitmap(&mut self, bitmap: &Vec<Vec<bool>>) -> Result<()> {
        debug!("开始绘制位图");
        let mut start_point: Option<(i32, i32)> = None;
        
        for y in 0..bitmap.len() {
            for x in 0..bitmap[y].len() {
                if bitmap[y][x] {
                    if start_point.is_none() {
                        start_point = Some((x as i32, y as i32));
                    }
                } else if let Some(start) = start_point {
                    // 找到一个连续线段的结束，画这条线
                    let end = (x as i32 - 1, y as i32);
                    self.draw_line_screen(start, end)?;
                    start_point = None;
                    sleep(Duration::from_millis(10));
                }
            }
            // 如果这一行结束时还有未画完的线段
            if let Some(start) = start_point {
                let end = (bitmap[y].len() as i32 - 1, y as i32);
                self.draw_line_screen(start, end)?;
                start_point = None;
            }
        }
        
        debug!("位图绘制完成");
        Ok(())
    }

    pub fn pen_down(&mut self) -> Result<()> {
        if let Some(device) = &mut self.device {
            debug!("笔落下");
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, 24, 4096),
                InputEvent::new(EventType::KEY, 330, 1),
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
            ])?;
            sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    pub fn pen_up(&mut self) -> Result<()> {
        if let Some(device) = &mut self.device {
            debug!("笔抬起");
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, 24, 0),
                InputEvent::new(EventType::KEY, 330, 0),
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
            ])?;
            sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    pub fn goto_xy_screen(&mut self, point: (i32, i32)) -> Result<()> {
        self.goto_xy(screen_to_input(point))
    }

    pub fn goto_xy(&mut self, (x, y): (i32, i32)) -> Result<()> {
        if let Some(device) = &mut self.device {
            debug!("笔移动到: ({}, {})", x, y);
            // 确保坐标在有效范围内
            let x = x.clamp(0, 15725) as i32;
            let y = y.clamp(0, 20967) as i32;
            
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, 0, x),
                InputEvent::new(EventType::ABSOLUTE, 1, y),
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
            ])?;
            sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    pub fn draw_point(&mut self, (x, y): (i32, i32)) -> Result<()> {
        debug!("笔开始绘制点: ({}, {})", x, y);
        self.pen_down()?;
        self.goto_xy((x, y))?;
        self.pen_up()?;
        debug!("笔结束绘制点");
        Ok(())
    }

    fn draw_char_bitmap(&mut self, bitmap: &Vec<Vec<bool>>, start_x: i32, start_y: i32) -> Result<()> {
        let mut last_point: Option<(i32, i32)> = None;
        
        // 先找到字的轮廓点
        let mut points = Vec::new();
        for y in 0..bitmap.len() {
            for x in 0..bitmap[y].len() {
                if bitmap[y][x] {
                    points.push((start_x + x as i32, start_y + y as i32));
                }
            }
        }
        
        // 按照笔画顺序连接点
        for &point in &points {
            if let Some(last) = last_point {
                // 如果两点之间距离不太远，就画一条线连接它们
                let dx = point.0 - last.0;
                let dy = point.1 - last.1;
                if dx * dx + dy * dy <= 25 { // 距离阈值
                    self.pen_up()?;
                    self.goto_xy_screen(last)?;
                    self.pen_down()?;
                    self.goto_xy_screen(point)?;
                    sleep(Duration::from_millis(5));
                }
            }
            last_point = Some(point);
        }
        
        Ok(())
    }

    pub fn get_char_strokes(&mut self, c: char) -> Result<Vec<Vec<(i32, i32)>>> {
        info!("开始获取字符 '{}' 的笔画", c);
        
        // 初始化 FreeType
        let library = match Library::init() {
            Ok(lib) => {
                info!("FreeType 库初始化成功");
                lib
            },
            Err(e) => {
                error!("FreeType 库初始化失败: {}", e);
                return Err(anyhow::anyhow!("FreeType 初始化失败"));
            }
        };
        
        // 从嵌入的资源中加载字体
        let font_data = match Asset::get("LXGWWenKaiScreen-Regular.ttf") {
            Some(data) => {
                info!("从嵌入资源加载字体数据成功");
                data.data.to_vec()
            },
            None => {
                error!("无法从嵌入资源加载字体");
                return Err(anyhow::anyhow!("无法加载字体文件"));
            }
        };
        
        // 从内存加载字体
        let face = match library.new_memory_face(font_data, 0) {
            Ok(face) => {
                info!("字体加载成功");
                face
            },
            Err(e) => {
                error!("字体加载失败: {}", e);
                return Err(anyhow::anyhow!("字体加载失败"));
            }
        };
        
        info!("设置字体大小");
        if let Err(e) = face.set_char_size(0, 32*64, 96, 96) {
            error!("设置字体大小失败: {}", e);
            return Err(anyhow::anyhow!("设置字体大小失败"));
        }
        
        // 加载字符
        info!("加载字符 '{}'", c);
        if let Err(e) = face.load_char(c as usize, freetype::face::LoadFlag::NO_SCALE) {
            error!("加载字符失败: {}", e);
            return Err(anyhow::anyhow!("加载字符失败"));
        }
        
        let glyph = face.glyph();
        let outline = match glyph.outline() {
            Some(o) => {
                info!("获取字符轮廓成功");
                o
            },
            None => {
                error!("无法获取字符轮廓");
                return Err(anyhow::anyhow!("无法获取字符轮廓"));
            }
        };
        
        let mut strokes = Vec::new();
        let mut current_stroke = Vec::new();
        
        // 获取轮廓点
        let points = outline.points();
        let tags = outline.tags();
        let contours = outline.contours();
        
        info!("字符 '{}' 的轮廓信息:", c);
        info!("点数: {}", points.len());
        info!("轮廓数: {}", contours.len());
        
        if points.is_empty() || contours.is_empty() {
            info!("字符 '{}' 没有笔画", c);
            return Ok(vec![]);
        }
        
        let mut start = 0;
        
        // 处理每个轮廓
        for (i, contour_end) in contours.iter().enumerate() {
            let end = *contour_end as usize + 1;
            current_stroke.clear();
            
            info!("处理轮廓 {}, 点范围: {} -> {}", i, start, end);
            
            // 处理当前轮廓的点
            for i in start..end {
                let point = points[i];
                let tag = tags[i];
                
                info!("处理点 {}: ({}, {}), tag: {}", i, point.x, point.y, tag);
                
                // 如果是轮廓起点或者控制点
                if tag & 0x1 != 0 {
                    current_stroke.push((point.x as i32, point.y as i32));
                    info!("添加点到当前笔画: ({}, {})", point.x, point.y);
                }
            }
            
            if !current_stroke.is_empty() {
                info!("添加笔画，点数: {}", current_stroke.len());
                strokes.push(current_stroke.clone());
            }
            
            start = end;
        }
        
        info!("字符 '{}' 的笔画提取完成，共 {} 个笔画", c, strokes.len());
        for (i, stroke) in strokes.iter().enumerate() {
            info!("笔画 {}: {} 个点", i, stroke.len());
            if !stroke.is_empty() {
                info!("起点: {:?}, 终点: {:?}", stroke.first(), stroke.last());
            }
        }
        
        Ok(strokes)
    }

    // 添加一个测试函数
    pub fn test_draw(&mut self) -> Result<()> {
        debug!("开始测试绘制");
        
        // 画一个简单的正方形
        let points = vec![
            (1000, 1000),
            (2000, 1000),
            (2000, 2000),
            (1000, 2000),
            (1000, 1000),
        ];

        self.pen_up()?;
        for (i, &(x, y)) in points.iter().enumerate() {
            if i == 0 {
                self.goto_xy((x, y))?;
                self.pen_down()?;
            } else {
                self.goto_xy((x, y))?;
            }
            sleep(Duration::from_millis(100));
        }
        self.pen_up()?;

        debug!("测试绘制完成");
        Ok(())
    }
}

fn screen_to_input(point: (i32, i32)) -> (i32, i32) {
    let (x, y) = point;
    // 坐标转换
    let x_normalized = x as f32 / REMARKABLE_WIDTH as f32;
    let y_normalized = y as f32 / REMARKABLE_HEIGHT as f32;
    
    let x_input = (x_normalized * INPUT_WIDTH as f32) as i32;
    let y_input = (y_normalized * INPUT_HEIGHT as f32) as i32;
    
    (x_input, y_input)
}
