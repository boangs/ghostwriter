use anyhow::Result;
use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use nix::sys::select::{select, FdSet};
use nix::sys::time::TimeVal;

const TOUCH_INPUT_DEVICE: &str = "/dev/input/event0";  // 尝试其他输入设备

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
    input_device: Option<File>,
    last_x: i32,
    last_y: i32,
    touch_started: bool,
    touch_complete: bool,
}

impl Touch {
    pub fn new(no_draw: bool) -> Self {
        println!("尝试打开触摸设备: {}", TOUCH_INPUT_DEVICE);
        let input_device = if !no_draw {
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
        
        Self { 
            no_draw,
            input_device,
            last_x: 0,
            last_y: 0,
            touch_started: false,
            touch_complete: false,
        }
    }

    pub fn wait_for_touch(&mut self) -> Result<bool> {
        if let Some(device) = &mut self.input_device {
            let fd = device.as_raw_fd();
            
            let mut fd_set = FdSet::new();
            fd_set.insert(fd);
            
            let mut timeout = TimeVal::new(0, 0);
            
            match select(fd + 1, Some(&mut fd_set), None, None, Some(&mut timeout)) {
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
                    
                    if device.read_exact(event_slice).is_ok() {
                        match event.type_ {
                            3 => {  // EV_ABS
                                match event.code {
                                    0 => {  // ABS_X
                                        self.last_x = event.value;
                                        println!("X: {}", self.last_x);
                                    },
                                    1 => {  // ABS_Y
                                        self.last_y = event.value;
                                        println!("Y: {}", self.last_y);
                                    },
                                    24 => {  // ABS_PRESSURE
                                        if event.value > 0 {
                                            self.touch_started = true;
                                        } else {
                                            self.touch_complete = true;
                                        }
                                    },
                                    _ => {}
                                }
                            },
                            0 => {  // EV_SYN
                                if self.touch_complete {
                                    // 检查是否在右上角区域
                                    if self.last_x > 1200 && self.last_y < 200 {
                                        return Ok(true);
                                    }
                                    self.touch_started = false;
                                    self.touch_complete = false;
                                }
                            },
                            _ => {}
                        }
                    }
                }
                Ok(_) => (),
                Err(e) => println!("Select error: {}", e),
            }
        }
        Ok(false)
    }
}
