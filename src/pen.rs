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

    pub fn pen_up(&mut self) -> Result<()> {
        if let Some(device) = &self.device {
            let events = vec![
                InputEvent::new(EventType::ABSOLUTE, 0x18, 0),
                InputEvent::new(EventType::SYNCHRONIZATION, 0x00, 0),
            ];
            for event in events {
                device.send_events(&[event])?;
            }
        }
        Ok(())
    }

    pub fn pen_down(&mut self) -> Result<()> {
        if let Some(device) = &self.device {
            let events = vec![
                InputEvent::new(EventType::ABSOLUTE, 0x18, 1),
                InputEvent::new(EventType::SYNCHRONIZATION, 0x00, 0),
            ];
            for event in events {
                device.send_events(&[event])?;
            }
        }
        Ok(())
    }

    pub fn goto_xy(&mut self, (x, y): (i32, i32)) -> Result<()> {
        if let Some(device) = &self.device {
            let events = vec![
                InputEvent::new(EventType::ABSOLUTE, 0x00, x as i32),
                InputEvent::new(EventType::ABSOLUTE, 0x01, y as i32),
                InputEvent::new(EventType::SYNCHRONIZATION, 0x00, 0),
            ];
            for event in events {
                device.send_events(&[event])?;
            }
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
