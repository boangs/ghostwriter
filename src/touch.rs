use anyhow::Result;
use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use nix::sys::select::{select, FdSet};
use nix::sys::time::TimeVal;

const TOUCH_INPUT_DEVICE: &str = "/dev/input/touchscreen0";

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
                    println!("打开触摸设备失败: {} (errno={})", 
                        e, e.raw_os_error().unwrap_or(-1));
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
        }
    }

    pub fn wait_for_touch(&mut self) -> Result<bool> {
        if let Some(device) = &mut self.input_device {
            let fd = device.as_raw_fd();
            let mut fd_set = FdSet::new();
            fd_set.insert(fd);
            
            let mut timeout = TimeVal::new(0, 100_000);  // 100ms timeout
            
            match select(
                fd + 1,
                Some(&mut fd_set),
                None,
                None,
                Some(&mut timeout),
            ) {
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
                        println!("触摸事件: type={}, code={}, value={}", 
                            event.type_, event.code, event.value);
                        
                        // 根据事件类型处理
                        match event.type_ {
                            0 => {  // EV_SYN
                                println!("同步事件");
                            },
                            1 => {  // EV_KEY
                                println!("按键事件: {}", event.value);
                            },
                            3 => {  // EV_ABS
                                match event.code {
                                    0 => {  // ABS_X
                                        self.last_x = event.value;
                                        println!("X坐标: {}", self.last_x);
                                    },
                                    1 => {  // ABS_Y
                                        self.last_y = event.value;
                                        println!("Y坐标: {}", self.last_y);
                                    },
                                    _ => {}
                                }
                                
                                // 检查是否是右上角的触摸
                                if self.last_x > 1300 && self.last_y < 200 {
                                    println!("检测到右上角触摸！({}, {})", self.last_x, self.last_y);
                                    return Ok(true);
                                }
                            },
                            _ => {}
                        }
                    }
                }
                Ok(_) => (),  // 超时，继续等待
                Err(e) => println!("Select error: {}", e),
            }
        } else {
            println!("触摸设备未打开");
        }
        Ok(false)
    }
}
