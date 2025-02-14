use anyhow::Result;
use evdev::{Device, EventType, InputEvent};
use std::thread::sleep;
use std::time::Duration;
use rusttype::{Font, Scale, Point};

const INPUT_WIDTH: usize = 15725;
const INPUT_HEIGHT: usize = 20966;

const REMARKABLE_WIDTH: u32 = 768;
const REMARKABLE_HEIGHT: u32 = 1024;

pub struct Pen {
    device: Option<Device>,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        let device = if no_draw {
            None
        } else {
            Some(Device::open("/dev/input/touchscreen0").unwrap())
        };

        Self { device }
    }

    pub fn draw_line_screen(&mut self, p1: (i32, i32), p2: (i32, i32)) -> Result<()> {
        self.draw_line(screen_to_input(p1), screen_to_input(p2))
    }

    pub fn draw_line(&mut self, (x1, y1): (i32, i32), (x2, y2): (i32, i32)) -> Result<()> {
        let length = ((x2 as f32 - x1 as f32).powf(2.0) + (y2 as f32 - y1 as f32).powf(2.0)).sqrt();
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
                    self.goto_xy_screen((x as i32 + 1, y as i32))?;
                } else if is_pen_down {
                    self.pen_up()?;
                    is_pen_down = false;
                    sleep(Duration::from_millis(1));
                }
            }
            self.pen_up()?;
            is_pen_down = false;
            sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    pub fn draw_text(&mut self, text: &str, position: (i32, i32), size: f32) -> Result<()> {
        // 加载字体
        let font_data = include_bytes!("../assets/DejaVuSans.ttf");
        let font = Font::try_from_bytes(font_data).unwrap();
        
        // 设置字体大小
        let scale = Scale::uniform(size);
        
        // 计算每个字符的布局
        let v_metrics = font.v_metrics(scale);
        let glyphs: Vec<_> = font.layout(text, scale, Point { 
            x: position.0 as f32, 
            y: position.1 as f32 + v_metrics.ascent 
        }).collect();
        
        // 遍历每个字形并绘制
        for glyph in glyphs {
            if let Some(outline) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    if v > 0.1 {
                        let x = outline.min.x as i32 + x as i32;
                        let y = outline.min.y as i32 + y as i32;
                        if let Err(e) = self.goto_xy_screen((x, y)) {
                            eprintln!("Error moving pen: {:?}", e);
                            return;
                        }
                        if let Err(e) = self.pen_down() {
                            eprintln!("Error putting pen down: {:?}", e);
                            return;
                        }
                        if let Err(e) = self.goto_xy_screen((x + 1, y)) {
                            eprintln!("Error drawing pixel: {:?}", e);
                            return;
                        }
                        if let Err(e) = self.pen_up() {
                            eprintln!("Error lifting pen: {:?}", e);
                            return;
                        }
                    }
                });
            }
        }
        
        Ok(())
    }

    pub fn pen_down(&mut self) -> Result<()> {
        if let Some(device) = &self.device {
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, ABS_PRESSURE, 1000),
                InputEvent::new(EventType::SYNCHRONIZATION, SYN_REPORT, 0),
            ])?;
        }
        Ok(())
    }

    pub fn pen_up(&mut self) -> Result<()> {
        if let Some(device) = &self.device {
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, ABS_PRESSURE, 0),
                InputEvent::new(EventType::SYNCHRONIZATION, SYN_REPORT, 0),
            ])?;
        }
        Ok(())
    }

    pub fn goto_xy(&mut self, (x, y): (i32, i32)) -> Result<()> {
        if let Some(device) = &self.device {
            device.send_events(&[
                InputEvent::new(EventType::ABSOLUTE, ABS_X, x as i32),
                InputEvent::new(EventType::ABSOLUTE, ABS_Y, y as i32),
                InputEvent::new(EventType::SYNCHRONIZATION, SYN_REPORT, 0),
            ])?;
        }
        Ok(())
    }

    pub fn goto_xy_screen(&mut self, (x, y): (i32, i32)) -> Result<()> {
        self.goto_xy(screen_to_input((x, y)))
    }
}

fn screen_to_input((x, y): (i32, i32)) -> (i32, i32) {
    (
        (x as f32 * INPUT_WIDTH as f32 / REMARKABLE_WIDTH as f32) as i32,
        (y as f32 * INPUT_HEIGHT as f32 / REMARKABLE_HEIGHT as f32) as i32,
    )
}

const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_PRESSURE: u16 = 0x18;
const SYN_REPORT: u16 = 0x00;
