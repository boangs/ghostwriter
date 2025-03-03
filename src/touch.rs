use anyhow::Result;
use evdev::{Device, EventType, InputEvent};
use log::{debug, trace, info};

use std::thread::sleep;
use std::time::Duration;

// Device to virtual coordinate conversion
const INPUT_WIDTH: u16 = 1404;
const INPUT_HEIGHT: u16 = 1872;
const REMARKABLE_WIDTH: u16 = 768;
const REMARKABLE_HEIGHT: u16 = 1024;

// Event codes
const ABS_MT_SLOT: u16 = 47;
const ABS_MT_TOUCH_MAJOR: u16 = 48;
const ABS_MT_TOUCH_MINOR: u16 = 49;
const ABS_MT_ORIENTATION: u16 = 52;
const ABS_MT_POSITION_X: u16 = 53;
const ABS_MT_POSITION_Y: u16 = 54;
// const ABS_MT_TOOL_TYPE: u16 = 55;
const ABS_MT_TRACKING_ID: u16 = 57;
const ABS_MT_PRESSURE: u16 = 58;

pub struct Touch {
    device: Option<Device>,
}

impl Touch {
    pub fn new(no_touch: bool) -> Self {
        let device = if no_touch {
            None
        } else {
            Some(Device::open("/dev/input/touchscreen0").unwrap())
        };

        Self { device }
    }

    pub fn wait_for_trigger(&mut self) -> Result<()> {
        let mut position_x = 0;
        let mut position_y = 0;
        loop {
            for event in self.device.as_mut().unwrap().fetch_events().unwrap() {
                if event.code() == ABS_MT_POSITION_X {
                    position_x = event.value();
                }
                if event.code() == ABS_MT_POSITION_Y {
                    position_y = event.value();
                }
                if event.code() == ABS_MT_TRACKING_ID {
                    if event.value() == -1 {
                        info!("触摸释放坐标: ({}, {})", position_x, position_y);
                        if position_x > 1345 && position_y > 1815 {
                            info!("触发识别！");
                            return Ok(());
                        }
                    } else {
                        info!("触摸坐标: ({}, {})", position_x, position_y);
                    }
                }
            }
        }
    }

    pub fn touch_start(&mut self, xy: (i32, i32)) -> Result<()> {
        let (x, y) = screen_to_input(xy);
        if let Some(device) = &mut self.device {
            trace!("touch_start at ({}, {})", x, y);
            sleep(Duration::from_millis(100));
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_SLOT, 0),
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_TRACKING_ID, 1),
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_POSITION_X, x),
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_POSITION_Y, y),
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_PRESSURE, 81),
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_TOUCH_MAJOR, 17),
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_TOUCH_MINOR, 17),
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_ORIENTATION, 4),
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
            ])?;
            sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    pub fn touch_stop(&mut self) -> Result<()> {
        if let Some(device) = &mut self.device {
            trace!("touch_stop");
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_SLOT, 0),
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_TRACKING_ID, -1),
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
            ])?;
            sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    pub fn goto_xy(&mut self, xy: (i32, i32)) -> Result<()> {
        let (x, y) = screen_to_input(xy);
        if let Some(device) = &mut self.device {
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_SLOT, 0),
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_POSITION_X, x),
                InputEvent::new(EventType::ABSOLUTE, ABS_MT_POSITION_Y, y),
                InputEvent::new(EventType::SYNCHRONIZATION, 0, 0), // SYN_REPORT
            ])?;
        }
        Ok(())
    }
}

fn screen_to_input((x, y): (i32, i32)) -> (i32, i32) {
    // Swap and normalize the coordinates
    let x_normalized = x as f32 / REMARKABLE_WIDTH as f32;
    let y_normalized = y as f32 / REMARKABLE_HEIGHT as f32;

    let x_input = (x_normalized * INPUT_WIDTH as f32) as i32;
    let y_input = ((1.0 - y_normalized) * INPUT_HEIGHT as f32) as i32;
    (x_input, y_input)
}
