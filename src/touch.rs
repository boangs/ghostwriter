use anyhow::Result;

pub struct Touch;

impl Touch {
    pub fn new(_no_draw: bool) -> Self {
        Self
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
}
