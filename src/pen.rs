use anyhow::Result;
use evdev::{Device, EventType, InputEvent};
use log::debug;
use std::thread::sleep;
use std::time::Duration;
use freetype::Library;

const INPUT_WIDTH: i32 = 15725;
const INPUT_HEIGHT: i32 = 20967;
const REMARKABLE_WIDTH: i32 = 1620;
const REMARKABLE_HEIGHT: i32 = 2160;

pub struct Pen {
    device: Option<Device>,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        let device = if no_draw {
            None
        } else {
            Some(Device::open("/dev/input/event2").unwrap())
        };
        Self { device }
    }

    pub fn write_text(&mut self, text: &str) -> Result<()> {
        debug!("开始书写文本: {}", text);
        let start_x = 80;
        let start_y = 50;
        let char_width = 35;
        let line_height = 40;
        let mut current_x = start_x;
        let mut current_y = start_y;

        for c in text.chars() {
            // 如果到达行尾，换行
            if current_x > 1050 {
                current_x = start_x;
                current_y += line_height;
            }

            // 获取字符的笔画信息
            if let Ok(strokes) = get_char_strokes(c) {
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
            device.send_events(&[
                InputEvent::new(EventType::KEY, 320, 1), // BTN_TOOL_PEN
                InputEvent::new(EventType::KEY, 330, 1), // BTN_TOUCH
                InputEvent::new(EventType::ABSOLUTE, 24, 2400), // ABS_PRESSURE (max pressure)
                InputEvent::new(EventType::ABSOLUTE, 25, 0), // ABS_DISTANCE
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
            ])?;
        }
        Ok(())
    }

    pub fn pen_up(&mut self) -> Result<()> {
        if let Some(device) = &mut self.device {
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, 24, 0), // ABS_PRESSURE
                InputEvent::new(EventType::ABSOLUTE, 25, 100), // ABS_DISTANCE
                InputEvent::new(EventType::KEY, 330, 0),     // BTN_TOUCH
                InputEvent::new(EventType::KEY, 320, 0),     // BTN_TOOL_PEN
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
            ])?;
        }
        Ok(())
    }

    pub fn goto_xy_screen(&mut self, point: (i32, i32)) -> Result<()> {
        self.goto_xy(screen_to_input(point))
    }

    pub fn goto_xy(&mut self, (x, y): (i32, i32)) -> Result<()> {
        if let Some(device) = &mut self.device {
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, 0, x),        // ABS_X
                InputEvent::new(EventType::ABSOLUTE, 1, y),        // ABS_Y
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
            ])?;
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

pub fn get_char_strokes(c: char) -> Result<Vec<Vec<(i32, i32)>>> {
    let library = Library::init()?;
    
    // 加载我们的字体
    let face = library.new_face("assets/LXGWWenKaiScreen-Regular.ttf", 0)?;
    face.set_char_size(0, 16*64, 96, 96)?;
    
    // 加载字符
    face.load_char(c as usize, freetype::face::LoadFlag::NO_SCALE)?;
    let glyph = face.glyph();
    let outline = glyph.outline().ok_or(anyhow::anyhow!("No outline found"))?;
    
    let mut strokes = Vec::new();
    let mut current_stroke = Vec::new();
    
    // 使用 FreeType 的轮廓遍历函数
    outline.iter_segments(|segment| {
        match segment {
            freetype::outline::Segment::MoveTo(point) => {
                if !current_stroke.is_empty() {
                    strokes.push(current_stroke.clone());
                    current_stroke.clear();
                }
                current_stroke.push((point.x as i32, point.y as i32));
            },
            freetype::outline::Segment::LineTo(point) => {
                current_stroke.push((point.x as i32, point.y as i32));
            },
            freetype::outline::Segment::ConicTo(control, point) => {
                let steps = 10;
                let start = current_stroke.last().unwrap();
                for i in 1..=steps {
                    let t = i as f32 / steps as f32;
                    let x = (1.0 - t).powi(2) * start.0 as f32 
                        + 2.0 * (1.0 - t) * t * control.x as f32 
                        + t.powi(2) * point.x as f32;
                    let y = (1.0 - t).powi(2) * start.1 as f32 
                        + 2.0 * (1.0 - t) * t * control.y as f32 
                        + t.powi(2) * point.y as f32;
                    current_stroke.push((x as i32, y as i32));
                }
            },
            freetype::outline::Segment::CubicTo(control1, control2, point) => {
                let steps = 10;
                let start = current_stroke.last().unwrap();
                for i in 1..=steps {
                    let t = i as f32 / steps as f32;
                    let mt = 1.0 - t;
                    let x = mt.powi(3) * start.0 as f32
                        + 3.0 * mt.powi(2) * t * control1.x as f32
                        + 3.0 * mt * t.powi(2) * control2.x as f32
                        + t.powi(3) * point.x as f32;
                    let y = mt.powi(3) * start.1 as f32
                        + 3.0 * mt.powi(2) * t * control1.y as f32
                        + 3.0 * mt * t.powi(2) * control2.y as f32
                        + t.powi(3) * point.y as f32;
                    current_stroke.push((x as i32, y as i32));
                }
            }
        }
        true
    })?;
    
    if !current_stroke.is_empty() {
        strokes.push(current_stroke);
    }
    
    Ok(strokes)
}
