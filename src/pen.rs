use anyhow::Result;
use evdev::{Device, EventType, InputEvent};
use freetype::Library;
use log::{debug, error, info};
use std::thread::sleep;
use std::time::Duration;
use crate::util::Asset;
use crate::constants::{INPUT_WIDTH, INPUT_HEIGHT, REMARKABLE_WIDTH, REMARKABLE_HEIGHT};

pub struct Pen {
    device: Option<Device>,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        Self {
            device: if no_draw { None } else { Some(Device::open("/dev/input/event2").unwrap()) },
        }
    }

    pub fn pen_down(&mut self) -> Result<()> {
        if let Some(ref mut device) = self.device {
            let events = vec![
                InputEvent::new(EventType::KEY, 320, 1),     // BTN_TOOL_PEN
                InputEvent::new(EventType::KEY, 330, 1),     // BTN_TOUCH
                InputEvent::new(EventType::ABSOLUTE, 24, 1200), // ABS_PRESSURE (max pressure)
                InputEvent::new(EventType::ABSOLUTE, 25, 0),    // ABS_DISTANCE
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
            ];
            for event in events {
                device.send_events(&[event])?;
            }
        }
        Ok(())
    }

    pub fn pen_up(&mut self) -> Result<()> {
        if let Some(ref mut device) = self.device {
            let events = vec![
                InputEvent::new(EventType::ABSOLUTE, 24, 0),    // ABS_PRESSURE
                InputEvent::new(EventType::ABSOLUTE, 25, 100),  // ABS_DISTANCE
                InputEvent::new(EventType::KEY, 330, 0),        // BTN_TOUCH
                InputEvent::new(EventType::KEY, 320, 0),        // BTN_TOOL_PEN
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
            ];
            for event in events {
                device.send_events(&[event])?;
            }
        }
        Ok(())
    }

    pub fn goto_xy(&mut self, (x, y): (i32, i32)) -> Result<()> {
        // 转换屏幕坐标到输入设备坐标
        let (input_x, input_y) = screen_to_input((x, y));
        
        if let Some(device) = &mut self.device {
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, 0, input_x),        // ABS_X
                InputEvent::new(EventType::ABSOLUTE, 1, input_y),        // ABS_Y
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),       // SYN_REPORT
            ])?;
        }
        Ok(())
    }

    pub fn get_char_strokes(&self, c: char) -> Result<Vec<Vec<(i32, i32)>>> {
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

    pub fn draw_bitmap(&mut self, bitmap: &Vec<Vec<bool>>) -> Result<()> {
        let scale_x = INPUT_WIDTH as f32 / bitmap[0].len() as f32;
        let scale_y = INPUT_HEIGHT as f32 / bitmap.len() as f32;
        
        for (y, row) in bitmap.iter().enumerate() {
            for (x, &pixel) in row.iter().enumerate() {
                if pixel {
                    let x_pos = (x as f32 * scale_x) as i32;
                    let y_pos = (y as f32 * scale_y) as i32;
                    
                    self.pen_down()?;
                    self.goto_xy((x_pos, y_pos))?;
                } else {
                    self.pen_up()?;
                }
            }
        }
        
        self.pen_up()?;
        Ok(())
    }
}

fn screen_to_input((x, y): (i32, i32)) -> (i32, i32) {
    let x_normalized = x as f32 / REMARKABLE_WIDTH as f32;
    let y_normalized = y as f32 / REMARKABLE_HEIGHT as f32;
    
    let x_input = (x_normalized * INPUT_WIDTH as f32) as i32;
    let y_input = (y_normalized * INPUT_HEIGHT as f32) as i32;
    
    (x_input, y_input)
}
