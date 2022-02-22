use crate::{
    drivers::pins::Pin,
    traits::Gint,
    typestates::pin::{
        self,
        PinId,
    }
};

pub trait Mode {}
pub trait NotAnd {}
pub trait NotOr {}
pub trait Set {}
pub struct NotSet;
impl Mode for NotSet {}
impl NotAnd for NotSet {}
impl NotOr for NotSet {}
pub struct And;
impl Mode for And {}
impl NotOr for And {}
impl Set for And {}
pub struct Or;
impl Mode for Or {}
impl NotAnd for Or {}
impl Set for Or {}

// pub enum Mode {
//     And,
//     Or,
// }

pub enum Active {
    High,
    Low,
}

pub enum Trigger {
    Edge,
    Level,
}

// This is a kind of build, but on itself.
// To accommodate the typestate changes, it's a consuming builder.

pub struct GroupInterrupt<GINT, MODE = NotSet>
where
    GINT: Gint,
    MODE: Mode,
{
    gint: GINT,
    #[allow(dead_code)]
    mode: MODE,
    trigger: Trigger,
}

impl<GINT> GroupInterrupt<GINT>
where
    GINT: Gint,
{
    pub fn new_edge_triggered(gint: GINT) -> GroupInterrupt<GINT, NotSet> {
        GroupInterrupt::new(gint, Trigger::Edge)
    }

    pub fn new_level_triggered(gint: GINT) -> GroupInterrupt<GINT, NotSet> {
        GroupInterrupt::new(gint, Trigger::Level)
    }

    pub fn new(gint: GINT, trigger: Trigger) -> GroupInterrupt<GINT, NotSet> {
        match trigger {
            Trigger::Edge => {
                gint.ctrl.modify(|_, w| w.trig().edge_triggered());
            },
            Trigger::Level => {
                gint.ctrl.modify(|_, w| w.trig().level_triggered());
            },
        };

        Self {
            gint,
            mode: NotSet,
            trigger,
        }
    }
}


impl<GINT, MODE> GroupInterrupt<GINT, MODE>
where
    GINT: Gint,
    MODE: Mode,
    MODE: NotAnd,
{
    pub fn or(
        self,
    ) -> GroupInterrupt<GINT, Or> {

        self.gint.ctrl.modify(|_, w| w.comb().or());

        GroupInterrupt {
            gint: self.gint,
            mode: Or,
            trigger: self.trigger,
        }
    }
}

impl<GINT, MODE> GroupInterrupt<GINT, MODE>
where
    GINT: Gint,
    MODE: Mode,
    MODE: NotOr,
{
    pub fn and(
        self,
    ) -> GroupInterrupt<GINT, And> {

        self.gint.ctrl.modify(|_, w| w.comb().and());

        GroupInterrupt {
            gint: self.gint,
            mode: And,
            trigger: self.trigger,
        }
    }
}

impl<GINT, MODE> GroupInterrupt<GINT, MODE>
where
    GINT: Gint,
    MODE: Mode,
    MODE: Set,
{

    pub fn on<PIO: PinId>(
        self,
        _pin: &Pin<PIO, pin::state::Gpio<pin::gpio::direction::Input>>,
        active: Active,
    ) -> GroupInterrupt<GINT, MODE> {

        match active {
            Active::Low =>  {
                self.gint.port_pol[PIO::PORT].modify(|r, w| unsafe {
                    w.pol().bits(r.pol().bits() & !PIO::MASK)
                });
            },
            Active::High =>  {
                self.gint.port_pol[PIO::PORT].modify(|r, w| unsafe {
                    w.pol().bits(r.pol().bits() | PIO::MASK)
                });
            },
        };

        self.gint.port_ena[PIO::PORT].modify(|r, w| unsafe {
            w.ena().bits(r.ena().bits() | PIO::MASK)
        });

        GroupInterrupt {
            gint: self.gint,
            mode: self.mode,
            trigger: self.trigger,
        }
    }

    pub fn on_high<PIO: PinId>(
        self,
        pin: &Pin<PIO, pin::state::Gpio<pin::gpio::direction::Input>>,
    ) -> GroupInterrupt<GINT, MODE> {
        self.on(pin, Active::High)
    }

    pub fn on_low<PIO: PinId>(
        self,
        pin: &Pin<PIO, pin::state::Gpio<pin::gpio::direction::Input>>,
    ) -> GroupInterrupt<GINT, MODE> {
        self.on(pin, Active::Low)
    }

    pub fn clear_interrupt(&self) {
        self.gint.ctrl.modify(|_, w| w.int().set_bit());
    }

}

impl<GINT, MODE> GroupInterrupt<GINT, MODE>
where
    GINT: Gint,
    MODE: Mode,
    MODE: NotAnd,
{
    pub fn or_on<PIO: PinId>(
        self,
        pin: &Pin<PIO, pin::state::Gpio<pin::gpio::direction::Input>>,
        active: Active,
    ) -> GroupInterrupt<GINT, Or> {

        self.or().on(pin, active)
    }

    pub fn or_on_high<PIO: PinId>(
        self,
        pin: &Pin<PIO, pin::state::Gpio<pin::gpio::direction::Input>>,
    ) -> GroupInterrupt<GINT, Or> {
        self.or_on(pin, Active::High)
    }

    pub fn or_on_low<PIO: PinId>(
        self,
        pin: &Pin<PIO, pin::state::Gpio<pin::gpio::direction::Input>>,
    ) -> GroupInterrupt<GINT, Or> {
        self.or_on(pin, Active::Low)
    }

}

impl<GINT, MODE> GroupInterrupt<GINT, MODE>
where
    GINT: Gint,
    MODE: Mode,
    MODE: NotOr,
{
    pub fn and_on<PIO: PinId>(
        self,
        pin: &Pin<PIO, pin::state::Gpio<pin::gpio::direction::Input>>,
        active: Active,
    ) -> GroupInterrupt<GINT, And> {

        self.and().on(pin, active)
    }

    pub fn and_on_high<PIO: PinId>(
        self,
        pin: &Pin<PIO, pin::state::Gpio<pin::gpio::direction::Input>>,
    ) -> GroupInterrupt<GINT, And> {
        self.and_on(pin, Active::High)
    }

    pub fn and_on_low<PIO: PinId>(
        self,
        pin: &Pin<PIO, pin::state::Gpio<pin::gpio::direction::Input>>,
    ) -> GroupInterrupt<GINT, And> {
        self.and_on(pin, Active::Low)
    }

}

