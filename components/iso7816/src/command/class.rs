// There are four ranges:
// First Interindustry   0b000x_xxxx
// Further Interindustry 0b01xx_xxxx
// Reserved              0b001x_xxxx
// Proprietary           0b1xxx_xxxx
//
// For the interindustry ranges, class contains:
// - chaining (continues/last)
// - secure messaging indication (none, two standard, proprietary)
// - logical channel number

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Class {
    cla: u8,
    range: Range,
    // secure_messaging: SecureMessaging,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SecureMessaging {
    None = 0,
    Proprietary = 1,
    Standard = 2,
    Authenticated = 3,
    Unknown,
}

impl SecureMessaging {
    pub fn none(&self) -> bool {
        *self == SecureMessaging::None
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Chain {
    LastOrOnly,
    NotTheLast,
    Unknown,
}

impl Chain {
    #[inline]
    pub fn last_or_only(&self) -> bool {
        *self == Chain::LastOrOnly
    }

    #[inline]
    pub fn not_the_last(&self) -> bool {
        *self == Chain::NotTheLast
    }
}

impl Class {
    #[inline]
    pub fn into_inner(self) -> u8 {
        self.cla
    }

    #[inline]
    pub fn range(&self) -> Range {
        self.range
    }

    #[inline]
    pub fn secure_messaging(&self) -> SecureMessaging {
        match self.range {
            Range::Interindustry(which) => match which {
                Interindustry::First => {
                    match (self.cla >> 2) & 0b11 {
                        0b00 => SecureMessaging::None,
                        0b01 => SecureMessaging::Proprietary,
                        0b10 => SecureMessaging::Standard,
                        0b11 => SecureMessaging::Authenticated,
                        _ => unreachable!(),
                    }
                },
                Interindustry::Further => {
                    match (self.cla >> 5)  != 0 {
                        true => SecureMessaging::Standard,
                        false => SecureMessaging::None,
                    }
                }
                Interindustry::Reserved => SecureMessaging::Unknown,
            }
            _ => SecureMessaging::Unknown,
        }
    }

    #[inline]
    pub fn chain(&self) -> Chain {
        match self.range {
            Range::Interindustry(which) => match which {
                Interindustry::First | Interindustry::Further => {
                    if self.cla & (1 << 4) != 0 {
                        Chain::NotTheLast
                    } else {
                        Chain::LastOrOnly
                    }
                }
                _ => Chain::Unknown,
            }
            _ => Chain::Unknown,
        }
    }

    #[inline]
    pub fn channel(&self) -> Option<u8> {
        Some(match self.range() {
            Range::Interindustry(Interindustry::First) => {
                self.cla & 0b11
            }
            Range::Interindustry(Interindustry::Further) => {
                4 + self.cla & 0b111
            }
            _ => return None
        })
    }


}

impl core::convert::TryFrom<u8> for Class {
    type Error = InvalidClass;

    #[inline]
    fn try_from(cla: u8) -> Result<Self, Self::Error> {
        let range = Range::try_from(cla)?;
        Ok(Self { cla, range })
    }
}

// impl core::ops::Deref for Class {
//     type Target = u8;
//     fn deref(&self) -> &Self::Target {
//         &self.cla
//     }
// }

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Range {
    Interindustry(Interindustry),
    Proprietary,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Interindustry {
    First,
    Further,
    Reserved,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct InvalidClass {}

impl core::convert::TryFrom<u8> for Range {
    type Error = InvalidClass;

    #[inline]
    fn try_from(cla: u8) -> Result<Self, Self::Error> {
        if cla == 0xff {
            return Err(InvalidClass {})
        }

        let range = match cla >> 5 {
            0b000 => Range::Interindustry(Interindustry::First),
            0b010 | 0b011 => Range::Interindustry(Interindustry::Further),
            0b001 => Range::Interindustry(Interindustry::Reserved),
            _ => Range::Proprietary,
        };

        Ok(range)
    }
}
