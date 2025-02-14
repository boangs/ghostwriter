use anyhow::Result;

pub struct Touch;

impl Touch {
    pub fn new(_no_draw: bool) -> Self {
        Self
    }

    pub fn wait_for_trigger(&mut self) -> Result<()> {
        println!("Simulated trigger");
        Ok(())
    }
}
