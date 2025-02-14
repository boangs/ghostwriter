use anyhow::Result;

pub struct Keyboard {
    no_draw_progress: bool,
    progress_count: u32,
}

impl Keyboard {
    pub fn new(_no_draw: bool, no_draw_progress: bool) -> Self {
        Self {
            no_draw_progress,
            progress_count: 0,
        }
    }

    pub fn progress(&mut self) -> Result<()> {
        if !self.no_draw_progress {
            self.progress_count += 1;
            println!("Progress: {}", ".".repeat(self.progress_count as usize));
        }
        Ok(())
    }

    pub fn progress_end(&mut self) -> Result<()> {
        if !self.no_draw_progress {
            println!("Progress complete!");
            self.progress_count = 0;
        }
        Ok(())
    }

    pub fn key_cmd_body(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn string_to_keypresses(&mut self, _text: &str) -> Result<()> {
        Ok(())
    }
}
