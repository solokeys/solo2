#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Intensities {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
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
