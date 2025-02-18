use anyhow::Result;
use evdev::{Device, EventType, InputEvent};
use log::debug;
use std::thread::sleep;
use std::time::Duration;
use std::fs::File;
use std::io::Write;

const INPUT_WIDTH: usize = 15725;
const INPUT_HEIGHT: usize = 20966;

const REMARKABLE_WIDTH: u32 = 768;
const REMARKABLE_HEIGHT: u32 = 1024;

pub struct Pen {
    device: Option<Device>,
    framebuffer: Option<File>,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        let device = if no_draw {
            None
        } else {
            Some(Device::open("/dev/input/touchscreen0").unwrap())
        };
        
        let framebuffer = if no_draw {
            None
        } else {
            Some(File::options()
                .write(true)
                .open("/dev/fb0")
                .unwrap())
        };

        Self { 
            device,
            framebuffer 
        }
    }

    pub fn draw_line_screen(&mut self, p1: (i32, i32), p2: (i32, i32)) -> Result<()> {
        self.draw_line(screen_to_input(p1), screen_to_input(p2))
    }

    pub fn draw_line(&mut self, (x1, y1): (i32, i32), (x2, y2): (i32, i32)) -> Result<()> {
        // trace!("Drawing from ({}, {}) to ({}, {})", x1, y1, x2, y2);

        // We know this is a straight line
        // So figure out the length
        // Then divide it into enough steps to only go 10 units or so
        // Start at x1, y1
        // And then for each step add the right amount to x and y

        let length = ((x2 as f32 - x1 as f32).powf(2.0) + (y2 as f32 - y1 as f32).powf(2.0)).sqrt();
        // 5.0 is the maximum distance between points
        // If this is too small
        let steps = (length / 5.0).ceil() as i32;
        let dx = (x2 - x1) / steps;
        let dy = (y2 - y1) / steps;
        // trace!(
        //     "Drawing from ({}, {}) to ({}, {}) in {} steps",
        //     x1, y1, x2, y2, steps
        // );

        self.pen_up()?;
        self.goto_xy((x1, y1))?;
        self.pen_down()?;

        for i in 0..steps {
            let x = x1 + dx * i;
            let y = y1 + dy * i;
            self.goto_xy((x, y))?;
            // trace!("Drawing to point at ({}, {})", x, y);
        }

        self.pen_up()?;

        Ok(())
    }

    pub fn draw_bitmap(&mut self, bitmap: &Vec<Vec<bool>>) -> Result<()> {
        let mut is_pen_down = false;
        for (y, row) in bitmap.iter().enumerate() {
            for (x, &pixel) in row.iter().enumerate() {
                if pixel {
                    if !is_pen_down {
                        self.goto_xy_screen((x as i32, y as i32))?;
                        self.pen_down()?;
                        is_pen_down = true;
                        sleep(Duration::from_millis(1));
                    }
                    self.goto_xy_screen((x as i32, y as i32))?;
                } else if is_pen_down {
                    self.pen_up()?;
                    is_pen_down = false;
                    sleep(Duration::from_millis(1));
                }
            }
            if is_pen_down {
                self.pen_up()?;
                is_pen_down = false;
            }
            self.refresh_screen()?; // 每行结束刷新一次屏幕
            sleep(Duration::from_millis(5));
        }
        self.refresh_screen()?; // 最后再刷新一次
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

    fn refresh_screen(&mut self) -> Result<()> {
        if let Some(fb) = &mut self.framebuffer {
            fb.write_all(&[0])?; // 触发屏幕刷新
            fb.flush()?;
        }
        Ok(())
    }
}
fn screen_to_input((x, y): (i32, i32)) -> (i32, i32) {
    // Swap and normalize the coordinates
    let x_normalized = x as f32 / REMARKABLE_WIDTH as f32;
    let y_normalized = y as f32 / REMARKABLE_HEIGHT as f32;

    let x_input = ((1.0 - y_normalized) * INPUT_HEIGHT as f32) as i32;
    let y_input = (x_normalized * INPUT_WIDTH as f32) as i32;
    (x_input, y_input)
}
