use anyhow::Result;
use evdev::{Device, EventType, InputEvent};
use log::{debug, trace, info, error};

use std::thread::sleep;
use std::time::Duration;

// Device to virtual coordinate conversion
const INPUT_WIDTH: u16 = 1404;
const INPUT_HEIGHT: u16 = 1872;
const REMARKABLE_WIDTH: u16 = 1620;
const REMARKABLE_HEIGHT: u16 = 2160;

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
            info!("触摸功能已禁用");
            None
        } else {
            info!("尝试打开触摸设备...");
            match Device::open("/dev/input/event3") {
                Ok(dev) => {
                    info!("成功打开触摸设备");
                    info!("设备名称: {}", dev.name().unwrap_or("未知"));
                    info!("支持的事件类型:");
                    for ev_type in dev.supported_events() {
                        info!("  - {:?}", ev_type);
                    }
                    Some(dev)
                }
                Err(e) => {
                    error!("无法打开触摸设备 /dev/input/touchscreen0: {}", e);
                    error!("请检查设备是否存在并且有正确的权限");
                    error!("可以尝试: ls -l /dev/input/touchscreen0");
                    error!("或者: ls -l /dev/input/event*");
                    // 尝试列出所有可用的输入设备
                    if let Ok(entries) = std::fs::read_dir("/dev/input") {
                        info!("可用的输入设备:");
                        for entry in entries {
                            if let Ok(entry) = entry {
                                info!("  - {}", entry.path().display());
                                // 尝试打开每个 event 设备并获取信息
                                if let Some(name) = entry.file_name().to_str() {
                                    if name.starts_with("event") {
                                        if let Ok(test_dev) = Device::open(entry.path()) {
                                            info!("    设备名称: {}", test_dev.name().unwrap_or("未知"));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    None
                }
            }
        };

        Self { device }
    }

    pub fn wait_for_trigger(&mut self) -> Result<()> {
        let mut position_x = 0;
        let mut position_y = 0;
        
        let device = self.device.as_mut().ok_or_else(|| {
            anyhow::anyhow!("触摸设备未初始化")
        })?;
        
        info!("等待触摸事件...");
        loop {
            match device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        debug!("收到事件: type={:?}, code={}, value={}", event.event_type(), event.code(), event.value());
                        match event.event_type() {
                            EventType::ABSOLUTE => {
                                match event.code() {
                                    ABS_MT_POSITION_X => {
                                        position_x = event.value();
                                        info!("X坐标: {}", position_x);
                                    }
                                    ABS_MT_POSITION_Y => {
                                        position_y = event.value();
                                        info!("Y坐标: {}", position_y);
                                    }
                                    ABS_MT_TRACKING_ID => {
                                        if event.value() == -1 {
                                            info!("触摸释放坐标: ({}, {})", position_x, position_y);
                                            if position_x > 2040 && position_y < 35 {
                                                info!("触发识别！");
                                                return Ok(());
                                            }
                                        } else {
                                            info!("触摸坐标: ({}, {})", position_x, position_y);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    error!("读取触摸事件失败: {}", e);
                    return Err(anyhow::anyhow!("读取触摸事件失败: {}", e));
                }
            }
        }
    }

    pub fn touch_start(&mut self, xy: (i32, i32)) -> Result<()> {
        let (x, y) = screen_to_input(xy);
        if let Some(device) = &mut self.device {
            info!("touch_start at ({}, {})", x, y);
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
            info!("touch_stop");
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
