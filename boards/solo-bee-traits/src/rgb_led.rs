
pub trait RgbLed {

    /// Set all LEDs using (R||G||B) formatted word.
    fn hex(&mut self, intensity: u32) {
        self.set_red(((intensity & 0xff0000)>>16) as u8);
        self.set_green(((intensity & 0xff00)>>8) as u8);
        self.set_blue((intensity & 0xff) as u8);
    }

    /// Set the intensity for the red LED.  0 turns off the LED.
    fn set_red(&mut self, intensity: u8);

    /// Set the intensity for the green LED.
    fn set_green(&mut self, intensity: u8);

    /// Set the intensity for the blue LED.
    fn set_blue(&mut self, intensity: u8);
}