use anyhow::Result;
use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use nix::sys::select::{select, FdSet};
use nix::sys::time::TimeVal;

const TOUCH_INPUT_DEVICE: &str = "/dev/input/touchscreen0";
const PEN_INPUT_DEVICE: &str = "/dev/input/event2";  // 通常触控笔是另一个输入设备

#[repr(C)]
#[derive(Debug)]
struct InputEvent {
    tv_sec: usize,
    tv_usec: usize,
    type_: u16,
    code: u16,
    value: i32,
}

pub struct Touch {
    no_draw: bool,
    touch_device: Option<File>,
    pen_device: Option<File>,
    last_x: i32,
    last_y: i32,
    pen_pressure: i32,
    touch_started: bool,
    touch_complete: bool,
}

impl Touch {
    pub fn new(no_draw: bool) -> Self {
        println!("尝试打开触摸设备: {}", TOUCH_INPUT_DEVICE);
        let touch_device = if !no_draw {
            match File::open(TOUCH_INPUT_DEVICE) {
                Ok(file) => {
                    println!("成功打开触摸设备");
                    Some(file)
                },
                Err(e) => {
                    println!("打开触摸设备失败: {}", e);
                    None
                }
            }
        } else {
            None
        };

        println!("尝试打开触控笔设备: {}", PEN_INPUT_DEVICE);
        let pen_device = if !no_draw {
            match File::open(PEN_INPUT_DEVICE) {
                Ok(file) => {
                    println!("成功打开触控笔设备");
                    Some(file)
                },
                Err(e) => {
                    println!("打开触控笔设备失败: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        Self { 
            no_draw,
            touch_device,
            pen_device,
            last_x: 0,
            last_y: 0,
            pen_pressure: 0,
            touch_started: false,
            touch_complete: false,
        }
    }

    pub fn wait_for_touch(&mut self) -> Result<bool> {
        let mut fd_set = FdSet::new();
        let mut max_fd = 0;

        // 添加触摸设备到 fd_set
        if let Some(ref device) = self.touch_device {
            let fd = device.as_raw_fd();
            fd_set.insert(fd);
            max_fd = fd;
        }

        // 添加触控笔设备到 fd_set
        if let Some(ref device) = self.pen_device {
            let fd = device.as_raw_fd();
            fd_set.insert(fd);
            max_fd = max_fd.max(fd);
        }

        let mut timeout = TimeVal::new(0, 10000);  // 10ms timeout

        match select(max_fd + 1, Some(&mut fd_set), None, None, Some(&mut timeout)) {
            Ok(n) if n > 0 => {
                let mut event = InputEvent {
                    tv_sec: 0,
                    tv_usec: 0,
                    type_: 0,
                    code: 0,
                    value: 0,
                };

                let size = std::mem::size_of::<InputEvent>();
                let event_ptr = &mut event as *mut _ as *mut u8;
                let event_slice = unsafe {
                    std::slice::from_raw_parts_mut(event_ptr, size)
                };

                // 处理触摸事件
                if let Some(ref mut device) = self.touch_device {
                    if fd_set.contains(device.as_raw_fd()) {
                        if device.read_exact(event_slice).is_ok() {
                            self.process_event(&event, true);
                        }
                    }
                }

                // 处理触控笔事件
                if let Some(ref mut device) = self.pen_device {
                    if fd_set.contains(device.as_raw_fd()) {
                        if device.read_exact(event_slice).is_ok() {
                            self.process_event(&event, false);
                        }
                    }
                }
            }
            Ok(_) => (),
            Err(e) => println!("Select error: {}", e),
        }

        // 在处理触摸事件后检查位置
        if self.touch_complete && self.last_x > 1800 && self.last_y < 200 {
            println!("触发右上角操作！");
            self.touch_complete = false;  // 重置状态
            return Ok(true);
        }

        Ok(false)
    }

    fn process_event(&mut self, event: &InputEvent, is_touch: bool) {
        match event.type_ {
            3 => {  // EV_ABS
                if is_touch {
                    // 处理触摸事件
                    match event.code {
                        53 => {  // ABS_MT_POSITION_X
                            self.last_x = event.value;
                            println!("触摸 X坐标: {}", self.last_x);
                        },
                        54 => {  // ABS_MT_POSITION_Y
                            self.last_y = event.value;
                            println!("触摸 Y坐标: {}", self.last_y);
                        },
                        57 => {  // ABS_MT_TRACKING_ID
                            if event.value == -1 {
                                self.touch_complete = true;
                                println!("触摸结束");
                            } else {
                                self.touch_started = true;
                                println!("触摸开始");
                            }
                        },
                        _ => {}
                    }
                } else {
                    // 处理触控笔事件
                    match event.code {
                        0 => {  // ABS_X
                            self.last_x = event.value;
                            println!("笔 X坐标: {}", self.last_x);
                        },
                        1 => {  // ABS_Y
                            self.last_y = event.value;
                            println!("笔 Y坐标: {}", self.last_y);
                        },
                        24 => {  // ABS_PRESSURE
                            self.pen_pressure = event.value;
                            println!("笔压力: {}", self.pen_pressure);
                        },
                        _ => {}
                    }
                }
            },
            1 => {  // EV_KEY
                if !is_touch && event.code == 320 {  // BTN_TOUCH for pen
                    if event.value > 0 {
                        println!("笔尖接触屏幕");
                    } else {
                        println!("笔尖离开屏幕");
                    }
                }
            },
            _ => {}
        }
    }
}
