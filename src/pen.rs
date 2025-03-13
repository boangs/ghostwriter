use anyhow::Result;
use evdev::{Device, EventType, InputEvent, Key};
use crate::constants::{INPUT_WIDTH, INPUT_HEIGHT, REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use std::time::Duration;
use libc;
use std::io::Read;

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

    pub fn eraser_down(&mut self) -> Result<()> {
        if let Some(ref mut device) = self.device {
            let events = vec![
                InputEvent::new(EventType::KEY, 321, 1),     // BTN_TOOL_RUBBER
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

    pub fn eraser_up(&mut self) -> Result<()> {
        if let Some(ref mut device) = self.device {
            let events = vec![
                InputEvent::new(EventType::ABSOLUTE, 24, 0),    // ABS_PRESSURE
                InputEvent::new(EventType::ABSOLUTE, 25, 100),  // ABS_DISTANCE
                InputEvent::new(EventType::KEY, 330, 0),        // BTN_TOUCH
                InputEvent::new(EventType::KEY, 321, 0),        // BTN_TOOL_RUBBER
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

    pub fn check_real_eraser(&mut self) -> Result<bool> {
        // 简化方法：直接检查设备状态而不是等待事件
        // 这个方法不会阻塞，适合在绘制过程中频繁调用
        
        // 由于我们无法直接从 evdev 获取当前状态，我们使用一个简单的方法：
        // 创建一个新的设备实例，读取当前状态
        if let Ok(device) = Device::open("/dev/input/event2") {
            // 检查设备的当前状态
            if let Ok(state) = device.get_key_state() {
                // 检查 BTN_TOOL_RUBBER (321) 是否被按下
                if state.contains(&321) {
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }

    pub fn draw_bitmap(&mut self, bitmap: &Vec<Vec<bool>>) -> Result<()> {
        let scale_x = INPUT_WIDTH as f32 / bitmap[0].len() as f32;
        let scale_y = INPUT_HEIGHT as f32 / bitmap.len() as f32;
        let mut pen_state = false;  // 跟踪笔的状态
        
        for (y, row) in bitmap.iter().enumerate() {
            // 检查是否有橡皮擦接触
            if self.check_real_eraser()? {
                println!("检测到真实橡皮擦接触！");
                // 这里可以选择要做什么，比如：
                // - 停止当前绘制
                // - 记录这个事件
                // - 或者继续绘制
            }
            
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
