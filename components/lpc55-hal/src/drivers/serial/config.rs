use crate::time::Hertz;

#[derive(Clone,Copy,Debug)]
pub enum WordLength {
    DataBits7,
    DataBits8,
    DataBits9,
}

#[derive(Clone,Copy,Debug)]
pub enum Parity {
    ParityNone,
    ParityEven,
    ParityOdd,
}

#[derive(Clone,Copy,Debug)]
pub enum StopBits {
    #[doc = "1 stop bit"]
    STOP1,
    // #[doc = "0.5 stop bits"]
    // STOP0P5,
    #[doc = "2 stop bits"]
    STOP2,
    // #[doc = "1.5 stop bits"]
    // STOP1P5,
}

#[derive(Clone,Copy,Debug)]
pub struct Config {
    pub speed: Hertz,
    pub wordlength: WordLength,
    pub parity: Parity,
    pub stopbits: StopBits,
}

impl Config {
    pub fn speed<Speed: Into<Hertz>>(mut self, speed: Speed) -> Self {
        self.speed = speed.into();
        self
    }

    pub fn parity_none(mut self) -> Self {
        self.parity = Parity::ParityNone;
        self
    }

    pub fn parity_even(mut self) -> Self {
        self.parity = Parity::ParityEven;
        self
    }

    pub fn parity_odd(mut self) -> Self {
        self.parity = Parity::ParityOdd;
        self
    }

    pub fn wordlength_8(mut self) -> Self {
        self.wordlength = WordLength::DataBits8;
        self
    }

    pub fn wordlength_9(mut self) -> Self {
        self.wordlength = WordLength::DataBits9;
        self
    }

    pub fn stopbits(mut self, stopbits: StopBits) -> Self {
        self.stopbits = stopbits;
        self
    }
}

#[derive(Debug)]
pub struct InvalidConfig;

impl Default for Config {
    /// The default ist 9600(8N1)
    fn default() -> Config {
        Config {
            // speed: Hertz(19_200),
            speed: Hertz(9_600),
            wordlength: WordLength::DataBits8,
            parity: Parity::ParityNone,
            stopbits: StopBits::STOP1,
        }
    }
}
