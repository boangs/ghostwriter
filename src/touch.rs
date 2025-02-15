use anyhow::Result;
use std::fs::File;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use nix::sys::select::{select, FdSet};
use std::time::Duration;

const TOUCH_INPUT_DEVICE: &str = "/dev/input/event1";

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
}

impl Touch {
    pub fn new(no_draw: bool) -> Self {
        let input_device = if !no_draw {
            File::open(TOUCH_INPUT_DEVICE).ok()
        } else {
            None
        };
        
        Self { 
            no_draw,
            input_device,
        }
    }

    pub fn touch_start(&mut self, _xy: (i32, i32)) -> Result<()> {
        println!("Simulated touch start");
        Ok(())
    }

    pub fn touch_stop(&mut self) -> Result<()> {
        println!("Simulated touch stop");
        Ok(())
    }

    pub fn wait_for_trigger(&mut self) -> Result<()> {
        println!("Simulated trigger");
        Ok(())
    }

    pub fn wait_for_touch(&mut self) -> Result<bool> {
        if let Some(device) = &self.input_device {
            let fd = device.as_raw_fd();
            let mut fd_set = FdSet::new();
            fd_set.insert(fd);
            
            // 设置超时时间为 100ms
            let timeout = Duration::from_millis(100);
            let mut timeout = nix::sys::time::TimeVal::from(timeout);
            
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
                        // 检查是否是右上角的触摸
                        // TODO: 根据实际坐标范围调整
                        if event.type_ == 3 && event.code == 0 && event.value > 1300 {
                            return Ok(true);
                        }
                    }
                }
                Ok(_) => (),  // 超时，继续等待
                Err(e) => println!("Select error: {}", e),
            }
        }
        Ok(false)
    }
}
