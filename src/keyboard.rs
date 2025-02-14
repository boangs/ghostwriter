use anyhow::Result;

use std::collections::HashMap;
use std::{thread, time};

use evdev::{
    uinput::VirtualDevice, uinput::VirtualDeviceBuilder, AttributeSet, EventType, InputEvent, Key,
};

pub struct Keyboard {
    device: Option<VirtualDevice>,
    key_map: HashMap<char, (Key, bool)>,
    progress_count: u32,
    no_draw_progress: bool,
}

impl Keyboard {
    pub fn new(no_draw: bool, no_draw_progress: bool) -> Self {
        let device = if no_draw {
            None
        } else {
            Some(Self::create_virtual_device())
        };

        Self {
            device,
            key_map: Self::create_key_map(),
            progress_count: 0,
            no_draw_progress,
        }
    }

    fn create_virtual_device() -> VirtualDevice {
        let mut keys = AttributeSet::new();

        keys.insert(Key::KEY_A);
        keys.insert(Key::KEY_B);
        keys.insert(Key::KEY_C);
        keys.insert(Key::KEY_D);
        keys.insert(Key::KEY_E);
        keys.insert(Key::KEY_F);
        keys.insert(Key::KEY_G);
        keys.insert(Key::KEY_H);
        keys.insert(Key::KEY_I);
        keys.insert(Key::KEY_J);
        keys.insert(Key::KEY_K);
        keys.insert(Key::KEY_L);
        keys.insert(Key::KEY_M);
        keys.insert(Key::KEY_N);
        keys.insert(Key::KEY_O);
        keys.insert(Key::KEY_P);
        keys.insert(Key::KEY_Q);
        keys.insert(Key::KEY_R);
        keys.insert(Key::KEY_S);
        keys.insert(Key::KEY_T);
        keys.insert(Key::KEY_U);
        keys.insert(Key::KEY_V);
        keys.insert(Key::KEY_W);
        keys.insert(Key::KEY_X);
        keys.insert(Key::KEY_Y);
        keys.insert(Key::KEY_Z);

        keys.insert(Key::KEY_1);
        keys.insert(Key::KEY_2);
        keys.insert(Key::KEY_3);
        keys.insert(Key::KEY_4);
        keys.insert(Key::KEY_5);
        keys.insert(Key::KEY_6);
        keys.insert(Key::KEY_7);
        keys.insert(Key::KEY_8);
        keys.insert(Key::KEY_9);
        keys.insert(Key::KEY_0);

        // Add punctuation and special keys
        keys.insert(Key::KEY_SPACE);
        keys.insert(Key::KEY_ENTER);
        keys.insert(Key::KEY_TAB);
        keys.insert(Key::KEY_LEFTSHIFT);
        keys.insert(Key::KEY_MINUS);
        keys.insert(Key::KEY_EQUAL);
        keys.insert(Key::KEY_LEFTBRACE);
        keys.insert(Key::KEY_RIGHTBRACE);
        keys.insert(Key::KEY_BACKSLASH);
        keys.insert(Key::KEY_SEMICOLON);
        keys.insert(Key::KEY_APOSTROPHE);
        keys.insert(Key::KEY_GRAVE);
        keys.insert(Key::KEY_COMMA);
        keys.insert(Key::KEY_DOT);
        keys.insert(Key::KEY_SLASH);

        keys.insert(Key::KEY_BACKSPACE);
        keys.insert(Key::KEY_ESC);

        keys.insert(Key::KEY_LEFTCTRL);
        keys.insert(Key::KEY_LEFTALT);

        VirtualDeviceBuilder::new()
            .unwrap()
            .name("Virtual Keyboard")
            .with_keys(&keys)
            .unwrap()
            .build()
            .unwrap()
    }

    fn create_key_map() -> HashMap<char, (Key, bool)> {
        let mut key_map = HashMap::new();

        // Lowercase letters
        key_map.insert('a', (Key::KEY_A, false));
        key_map.insert('b', (Key::KEY_B, false));
        key_map.insert('c', (Key::KEY_C, false));
        key_map.insert('d', (Key::KEY_D, false));
        key_map.insert('e', (Key::KEY_E, false));
        key_map.insert('f', (Key::KEY_F, false));
        key_map.insert('g', (Key::KEY_G, false));
        key_map.insert('h', (Key::KEY_H, false));
        key_map.insert('i', (Key::KEY_I, false));
        key_map.insert('j', (Key::KEY_J, false));
        key_map.insert('k', (Key::KEY_K, false));
        key_map.insert('l', (Key::KEY_L, false));
        key_map.insert('m', (Key::KEY_M, false));
        key_map.insert('n', (Key::KEY_N, false));
        key_map.insert('o', (Key::KEY_O, false));
        key_map.insert('p', (Key::KEY_P, false));
        key_map.insert('q', (Key::KEY_Q, false));
        key_map.insert('r', (Key::KEY_R, false));
        key_map.insert('s', (Key::KEY_S, false));
        key_map.insert('t', (Key::KEY_T, false));
        key_map.insert('u', (Key::KEY_U, false));
        key_map.insert('v', (Key::KEY_V, false));
        key_map.insert('w', (Key::KEY_W, false));
        key_map.insert('x', (Key::KEY_X, false));
        key_map.insert('y', (Key::KEY_Y, false));
        key_map.insert('z', (Key::KEY_Z, false));

        // Uppercase letters
        key_map.insert('A', (Key::KEY_A, true));
        key_map.insert('B', (Key::KEY_B, true));
        key_map.insert('C', (Key::KEY_C, true));
        key_map.insert('D', (Key::KEY_D, true));
        key_map.insert('E', (Key::KEY_E, true));
        key_map.insert('F', (Key::KEY_F, true));
        key_map.insert('G', (Key::KEY_G, true));
        key_map.insert('H', (Key::KEY_H, true));
        key_map.insert('I', (Key::KEY_I, true));
        key_map.insert('J', (Key::KEY_J, true));
        key_map.insert('K', (Key::KEY_K, true));
        key_map.insert('L', (Key::KEY_L, true));
        key_map.insert('M', (Key::KEY_M, true));
        key_map.insert('N', (Key::KEY_N, true));
        key_map.insert('O', (Key::KEY_O, true));
        key_map.insert('P', (Key::KEY_P, true));
        key_map.insert('Q', (Key::KEY_Q, true));
        key_map.insert('R', (Key::KEY_R, true));
        key_map.insert('S', (Key::KEY_S, true));
        key_map.insert('T', (Key::KEY_T, true));
        key_map.insert('U', (Key::KEY_U, true));
        key_map.insert('V', (Key::KEY_V, true));
        key_map.insert('W', (Key::KEY_W, true));
        key_map.insert('X', (Key::KEY_X, true));
        key_map.insert('Y', (Key::KEY_Y, true));
        key_map.insert('Z', (Key::KEY_Z, true));

        // Numbers
        key_map.insert('0', (Key::KEY_0, false));
        key_map.insert('1', (Key::KEY_1, false));
        key_map.insert('2', (Key::KEY_2, false));
        key_map.insert('3', (Key::KEY_3, false));
        key_map.insert('4', (Key::KEY_4, false));
        key_map.insert('5', (Key::KEY_5, false));
        key_map.insert('6', (Key::KEY_6, false));
        key_map.insert('7', (Key::KEY_7, false));
        key_map.insert('8', (Key::KEY_8, false));
        key_map.insert('9', (Key::KEY_9, false));

        // Special characters
        key_map.insert('!', (Key::KEY_1, true));
        key_map.insert('@', (Key::KEY_2, true));
        key_map.insert('#', (Key::KEY_3, true));
        key_map.insert('$', (Key::KEY_4, true));
        key_map.insert('%', (Key::KEY_5, true));
        key_map.insert('^', (Key::KEY_6, true));
        key_map.insert('&', (Key::KEY_7, true));
        key_map.insert('*', (Key::KEY_8, true));
        key_map.insert('(', (Key::KEY_9, true));
        key_map.insert(')', (Key::KEY_0, true));
        key_map.insert('_', (Key::KEY_MINUS, true));
        key_map.insert('+', (Key::KEY_EQUAL, true));
        key_map.insert('{', (Key::KEY_LEFTBRACE, true));
        key_map.insert('}', (Key::KEY_RIGHTBRACE, true));
        key_map.insert('|', (Key::KEY_BACKSLASH, true));
        key_map.insert(':', (Key::KEY_SEMICOLON, true));
        key_map.insert('"', (Key::KEY_APOSTROPHE, true));
        key_map.insert('<', (Key::KEY_COMMA, true));
        key_map.insert('>', (Key::KEY_DOT, true));
        key_map.insert('?', (Key::KEY_SLASH, true));
        key_map.insert('~', (Key::KEY_GRAVE, true));

        // Common punctuation
        key_map.insert('-', (Key::KEY_MINUS, false));
        key_map.insert('=', (Key::KEY_EQUAL, false));
        key_map.insert('[', (Key::KEY_LEFTBRACE, false));
        key_map.insert(']', (Key::KEY_RIGHTBRACE, false));
        key_map.insert('\\', (Key::KEY_BACKSLASH, false));
        key_map.insert(';', (Key::KEY_SEMICOLON, false));
        key_map.insert('\'', (Key::KEY_APOSTROPHE, false));
        key_map.insert(',', (Key::KEY_COMMA, false));
        key_map.insert('.', (Key::KEY_DOT, false));
        key_map.insert('/', (Key::KEY_SLASH, false));
        key_map.insert('`', (Key::KEY_GRAVE, false));

        // Whitespace
        key_map.insert(' ', (Key::KEY_SPACE, false));
        key_map.insert('\t', (Key::KEY_TAB, false));
        key_map.insert('\n', (Key::KEY_ENTER, false));

        // Action keys, such as backspace, escape, ctrl, alt
        key_map.insert('\x08', (Key::KEY_BACKSPACE, false));
        key_map.insert('\x1b', (Key::KEY_ESC, false));

        key_map
    }

    pub fn key_down(&mut self, key: Key) -> Result<()> {
        if let Some(device) = &mut self.device {
            device.emit(&[(InputEvent::new(EventType::KEY, key.code(), 1))])?;
        }
        Ok(())
    }

    pub fn key_up(&mut self, key: Key) -> Result<()> {
        if let Some(device) = &mut self.device {
            device.emit(&[(InputEvent::new(EventType::KEY, key.code(), 0))])?;
        }
        Ok(())
    }

    pub fn string_to_keypresses(&mut self, input: &str) -> Result<(), evdev::Error> {
        if let Some(device) = &mut self.device {
            for c in input.chars() {
                if let Some(&(key, shift)) = self.key_map.get(&c) {
                    if shift {
                        // Press Shift
                        device.emit(&[InputEvent::new(
                            EventType::KEY,
                            Key::KEY_LEFTSHIFT.code(),
                            1,
                        )])?;
                    }

                    // Press key
                    device.emit(&[InputEvent::new(EventType::KEY, key.code(), 1)])?;

                    // Release key
                    device.emit(&[InputEvent::new(EventType::KEY, key.code(), 0)])?;

                    if shift {
                        // Release Shift
                        device.emit(&[InputEvent::new(
                            EventType::KEY,
                            Key::KEY_LEFTSHIFT.code(),
                            0,
                        )])?;
                    }

                    // Sync event
                    device.emit(&[InputEvent::new(EventType::SYNCHRONIZATION, 0, 0)])?;
                    thread::sleep(time::Duration::from_millis(10));
                }
            }
        }
        Ok(())
    }

    fn key_cmd(&mut self, button: &str, shift: bool) -> Result<()> {
        self.key_down(Key::KEY_LEFTCTRL)?;
        if shift {
            self.key_down(Key::KEY_LEFTSHIFT)?;
        }
        self.string_to_keypresses(button)?;
        if shift {
            self.key_up(Key::KEY_LEFTSHIFT)?;
        }
        self.key_up(Key::KEY_LEFTCTRL)?;
        Ok(())
    }

    pub fn key_cmd_title(&mut self) -> Result<()> {
        self.key_cmd("1", false)?;
        Ok(())
    }

    pub fn key_cmd_subheading(&mut self) -> Result<()> {
        self.key_cmd("2", false)?;
        Ok(())
    }

    pub fn key_cmd_body(&mut self) -> Result<()> {
        self.key_cmd("3", false)?;
        Ok(())
    }

    pub fn key_cmd_bullet(&mut self) -> Result<()> {
        self.key_cmd("4", false)?;
        Ok(())
    }

    pub fn progress(&mut self) -> Result<()> {
        if self.no_draw_progress {
            return Ok(());
        }
        self.string_to_keypresses(".")?;
        self.progress_count += 1;
        Ok(())
    }

    pub fn progress_end(&mut self) -> Result<()> {
        if self.no_draw_progress {
            return Ok(());
        }
        // Send a backspace for each progress
        for _ in 0..self.progress_count {
            self.string_to_keypresses("\x08")?;
        }
        self.progress_count = 0;
        Ok(())
    }
}
