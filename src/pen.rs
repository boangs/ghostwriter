use anyhow::Result;
use evdev::{Device, EventType, InputEvent};
use freetype::Library;
use log::{debug, error, info};
use std::thread::sleep;
use std::time::Duration;
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
        let char_width = 100;  // 字符间距
        let line_height = 150; // 行间距
        let mut current_x = start_x;
        let mut current_y = start_y;

        for c in text.chars() {
            // 获取字符的笔画
            let strokes = self.get_char_strokes(c)?;
            
            // 检查是否需要换行
            if current_x + char_width > REMARKABLE_WIDTH {
                current_y += line_height;
                current_x = start_x;
            }
            
            // 写入字符
            for stroke in strokes {
                // 移动到笔画起点
                self.pen_up()?;
                if let Some(&first_point) = stroke.first() {
                    let (x, y) = first_point;
                    self.goto_xy((x + current_x, y + current_y))?;
                    self.pen_down()?;
                    
                    // 绘制笔画的其余部分
                    for &(x, y) in stroke.iter().skip(1) {
                        self.goto_xy((x + current_x, y + current_y))?;
                    }
                }
                self.pen_up()?;
            }
            
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

    pub fn draw_bitmap(&mut self, bitmap: &Vec<Vec<bool>>) -> Result<()> {
        info!("开始绘制位图");
        for (y, row) in bitmap.iter().enumerate() {
            for (x, &pixel) in row.iter().enumerate() {
                if pixel {
                    self.goto_xy((x as i32, y as i32))?;
                }
            }
        }
        Ok(())
    }

    pub fn get_char_strokes(&mut self, c: char) -> Result<Vec<Vec<(i32, i32)>>> {
        info!("开始获取字符 '{}' 的笔画", c);
        
        let library = match Library::init() {
            Ok(lib) => lib,
            Err(e) => {
                error!("FreeType 库初始化失败: {}", e);
                return Err(anyhow::anyhow!("FreeType 初始化失败"));
            }
        };
        
        if let Some(font_data) = Asset::get("LXGWWenKaiScreen-Regular.ttf") {
            let face = library.new_memory_face(font_data.data.to_vec(), 0)?;
            face.set_char_size(0, 72 * 64, 96, 96)?;
            face.load_char(c as usize, freetype::face::LoadFlag::NO_SCALE)?;
            
            let glyph = face.glyph();
            let outline = glyph.outline().ok_or_else(|| anyhow::anyhow!("无法获取字符轮廓"))?;
            
            let points = outline.points();
            let tags = outline.tags();
            let contours = outline.contours();
            
            let mut strokes = Vec::new();
            let mut start: usize = 0;
            
            for end in contours.iter() {
                let mut current_stroke = Vec::new();
                let end_idx = *end as usize;
                
                for i in start..=end_idx {
                    let point = points[i];
                    let tag = tags[i];
                    
                    if tag & 0x1 != 0 {
                        let x = (point.x as f32 * 0.5) as i32;
                        let y = (point.y as f32 * 0.5) as i32;
                        current_stroke.push((x, y));
                    }
                }
                
                if !current_stroke.is_empty() {
                    strokes.push(current_stroke);
                }
                
                start = end_idx + 1;
            }
            
            Ok(strokes)
        } else {
            Err(anyhow::anyhow!("找不到字体文件"))
        }
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
