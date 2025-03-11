use anyhow::Result;
use evdev::{Device, EventType, InputEvent};
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

    pub fn is_eraser_touched(&mut self) -> Result<bool> {
        if let Some(ref mut device) = self.device {
            let mut events = [evdev::InputEvent::new(EventType::SYNCHRONIZATION, 0, 0); 16];
            if let Ok(num_events) = device.fetch_events(&mut events) {
                for event in events.iter().take(num_events) {
                    if event.event_type() == EventType::KEY && event.code() == 321 {  // BTN_TOOL_RUBBER
                        return Ok(event.value() != 0);
                    }
                }
            }
        }
        Ok(false)
    }

    pub fn pen_down(&mut self) -> Result<()> {
        if let Some(ref mut device) = self.device {
            let events = vec![
                InputEvent::new(EventType::KEY, 320, 1),     // BTN_TOOL_PEN
                InputEvent::new(EventType::KEY, 330, 1),     // BTN_TOUCH
                InputEvent::new(EventType::ABSOLUTE, 24, 2400), // ABS_PRESSURE (max pressure)
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

    pub fn draw_bitmap(&mut self, bitmap: &Vec<Vec<bool>>) -> Result<()> {
        let scale_x = INPUT_WIDTH as f32 / bitmap[0].len() as f32;
        let scale_y = INPUT_HEIGHT as f32 / bitmap.len() as f32;
        let mut pen_state = false;  // 跟踪笔的状态
        
        for (y, row) in bitmap.iter().enumerate() {
            for (x, &pixel) in row.iter().enumerate() {
                if pixel {
                    let x_pos = (x as f32 * scale_x) as i32;
                    let y_pos = (y as f32 * scale_y) as i32;
                    
                    if !pen_state {
                        self.pen_down()?;
                        pen_state = true;
                    }
                    self.goto_xy((x_pos, y_pos))?;
                } else if pen_state {
                    self.pen_up()?;
                    pen_state = false;
                }
            }
        }
        
        if pen_state {
            self.pen_up()?;
        }
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
