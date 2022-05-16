#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Intensities {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Color {
    Red,
    Green,
    Blue,
}

impl From<u32> for Intensities {
    // set all LEDs using (R||G||B) formatted word.
    fn from(hex: u32) -> Self{
        Intensities {
            red:   ((hex & 0xff_00_00) >> 16) as _,
            green: ((hex & 0x00_ff_00) >> 8) as _,
            blue:   (hex & 0x00_00_ff) as _,
        }
    }
}

impl Intensities {

    pub fn scale_by(&mut self, percent: &u8) -> Self {
        let scale: f32 = (percent / 100).into();
        Intensities {
            red: (self.red as f32 * scale) as u8,
            green: (self.green as f32 * scale) as u8,
            blue: (self.blue as f32 * scale) as u8
        }
    }
}


pub trait RgbLed {

    /// Set all LEDs
    fn set(&mut self, intensities: Intensities) {
        self.red(intensities.red);
        self.green(intensities.green);
        self.blue(intensities.blue);
    }

    /// Turn off all LEDs
    fn turn_off(&mut self) {
        self.set(0.into())
    }

    /// Set the intensity for the red LED.  0 turns off the LED.
    fn red(&mut self, intensity: u8);

    /// Set the intensity for the green LED.
    fn green(&mut self, intensity: u8);

    /// Set the intensity for the blue LED.
    fn blue(&mut self, intensity: u8);
}


pub const BLACK: Intensities = Intensities { red: 0, green: 0, blue: 0 };
pub const RED: Intensities = Intensities { red: u8::MAX, green: 0, blue: 0 };
pub const GREEN: Intensities = Intensities { red: 0, green: u8::MAX, blue: 0x02 };
pub const BLUE: Intensities = Intensities { red: 0, green: 0, blue: u8::MAX };
pub const TEAL: Intensities = Intensities { red: 0, green: u8::MAX, blue: 0x5a };
pub const ORANGE: Intensities = Intensities { red: u8::MAX, green: 0x7e, blue: 0 };
pub const WHITE: Intensities = Intensities { red: u8::MAX, green: u8::MAX, blue: u8::MAX };