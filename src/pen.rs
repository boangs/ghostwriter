use anyhow::Result;
use evdev::{Device, EventType, InputEvent};
use log::debug;
use std::thread::sleep;
use std::time::Duration;
use crate::util::svg_to_bitmap;

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
        let char_width = 30;
        let line_height = 40;
        let mut current_x = start_x;
        let mut current_y = start_y;

        for c in text.chars() {
            // 如果到达行尾，换行
            if current_x > 1050 {
                current_x = start_x;
                current_y += line_height;
            }

            // 将单个字符转换为 SVG，使用 LXGW WenKai Screen 字体
            let svg = format!(
                r#"<svg width='35' height='45' xmlns='http://www.w3.org/2000/svg'>
                    <text x='5' y='5' font-family='LXGW WenKai GB Screen' font-size='30'>{}</text>
                </svg>"#,
                c
            );
            
            // 获取字符的位图
            let bitmap = svg_to_bitmap(&svg, 35, 45)?;
            
            // 绘制这个字符的位图
            self.draw_char_bitmap(&bitmap, current_x, current_y)?;
            sleep(Duration::from_millis(10)); // 字符间停顿
            
            // 移动到下一个字符位置
            current_x += char_width;
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
                sleep(Duration::from_millis(10));
            }
        }
        
        debug!("位图绘制完成");
        Ok(())
    }

    // fn draw_dot(device: &mut Device, (x, y): (i32, i32)) -> Result<()> {
    //     // trace!("Drawing at ({}, {})", x, y);
    //     goto_xy(device, (x, y))?;
    //     pen_down(device)?;
    //
    //     // Wiggle a little bit
    //     for n in 0..2 {
    //         goto_xy(device, (x + n, y + n))?;
    //     }
    //
    //     pen_up(device)?;
    //
    //     // sleep for 5ms
    //     thread::sleep(time::Duration::from_millis(1));
    //
    //     Ok(())
    // }

    pub fn pen_down(&mut self) -> Result<()> {
        if let Some(device) = &mut self.device {
            device.send_events(&[
                InputEvent::new(EventType::KEY, 320, 1), // BTN_TOOL_PEN
                InputEvent::new(EventType::KEY, 330, 1), // BTN_TOUCH
                InputEvent::new(EventType::ABSOLUTE, 24, 2630), // ABS_PRESSURE (max pressure)
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
        self.pen_up()?;
        
        // 遍历位图中的每个像素
        for y in 0..bitmap.len() {
            let mut start_point: Option<(i32, i32)> = None;
            
            for x in 0..bitmap[y].len() {
                if bitmap[y][x] {
                    if start_point.is_none() {
                        // 找到这一行的第一个黑色像素
                        start_point = Some((start_x + x as i32, start_y + y as i32));
                    }
                } else if let Some(start) = start_point {
                    // 找到一个白色像素，结束当前线段
                    let end = (start_x + x as i32 - 1, start_y + y as i32);
                    self.draw_line_screen(start, end)?;
                    start_point = None;
                }
            }
            
            // 处理这一行最后的黑色像素
            if let Some(start) = start_point {
                let end = (start_x + bitmap[y].len() as i32 - 1, start_y + y as i32);
                self.draw_line_screen(start, end)?;
            }
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
