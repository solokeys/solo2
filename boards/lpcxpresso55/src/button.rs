
        // let user_button = pins.pio1_9
        //     .into_gpio_pin(&mut iocon, &mut gpio)
        //     .into_input()
        // ;

        // // easier to reach
        // let wakeup_button = pins.pio1_18
        //     .into_gpio_pin(&mut iocon, &mut gpio)
        //     .into_input()
        // ;

type UserButton = hal::Pin<pins::Pio1_9, pin::state::Gpio<pin::gpio::direction::Input>>;
type WakeupButton = hal::Pin<pins::Pio1_18, pin::state::Gpio<pin::gpio::direction::Input>>;

// type ButtonPress = GroupInterrupt<Gint0, hal::drivers::gint::Or>;
// type ButtonRelease = GroupInterrupt<Gint1, hal::drivers::gint::Or>;
