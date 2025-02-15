use anyhow::Result;
use std::fs::{File, read_dir};
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use nix::sys::select::{select, FdSet};
use nix::sys::time::TimeVal;
use std::path::Path;

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
        println!("扫描输入设备...");
        
        // 列出所有输入设备
        if let Ok(entries) = read_dir("/dev/input") {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if let Some(name) = path.file_name() {
                        if let Some(name_str) = name.to_str() {
                            // 尝试获取设备信息
                            if let Ok(mut file) = File::open(&path) {
                                let mut info = [0u8; 256];
                                unsafe {
                                    let result = libc::ioctl(
                                        file.as_raw_fd(),
                                        libc::EVIOCGNAME(info.len() as u32),
                                        info.as_mut_ptr() as *mut libc::c_void,
                                    );
                                    if result >= 0 {
                                        let device_name = String::from_utf8_lossy(&info[..result as usize]);
                                        println!("设备: {} - {}", name_str, device_name.trim_matches(char::from(0)));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 尝试打开 touchscreen0
        println!("尝试打开触摸屏设备: /dev/input/touchscreen0");
        let input_device = if !no_draw {
            match File::open("/dev/input/touchscreen0") {
                Ok(file) => {
                    println!("成功打开触摸屏设备");
                    Some(file)
                },
                Err(e) => {
                    println!("打开触摸屏设备失败: {}", e);
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
                        // 输出所有事件的详细信息
                        println!("输入事件: type={}, code={}, value={}", 
                            event.type_, event.code, event.value);
                    }
                }
                Ok(_) => (),  // 超时，继续等待
                Err(e) => println!("Select error: {}", e),
            }
        }
        Ok(false)
    }
}
