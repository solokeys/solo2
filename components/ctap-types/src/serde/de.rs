use serde::Deserialize;

use serde::de::{
    IntoDeserializer,
};

use super::error::{Error, Result};

/// Deserialize a message of type `T` from a byte slice. The unused portion (if any)
/// of the byte slice is returned for further usage
pub fn from_bytes<'a, T>(s: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_bytes(s);
    let t = T::deserialize(&mut deserializer)?;
    Ok(t)
}

/// Deserialize a message of type `T` from a byte slice. The unused portion (if any)
/// of the byte slice is returned for further usage
pub fn take_from_bytes<'a, T>(s: &'a [u8]) -> Result<(T, &'a [u8])>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_bytes(s);
    let t = T::deserialize(&mut deserializer)?;
    Ok((t, deserializer.input))
}

////////////////////////////////////////////////////////////////////////////////

// TODO: remove these allowances again later
// #![allow(unused_imports)]
// #![allow(unused_variables)]

use core::convert::TryInto;

use serde::de::{
    self,
    DeserializeSeed,
    Visitor,
};

/// A structure for deserializing a ctapcbor message.
pub struct Deserializer<'de> {
    // This string starts with the input data and characters are truncated off
    // the beginning as data is parsed.
    pub(crate) input: &'de [u8],
}

impl<'de> Deserializer<'de> {
    /// Obtain a Deserializer from a slice of bytes
    pub fn from_bytes(input: &'de [u8]) -> Self {
        Deserializer { input }
    }
}

impl<'de> Deserializer<'de> {
    fn try_take_n(&mut self, count: usize) -> Result<&'de [u8]> {
        if self.input.len() >= count {
            let (a, b) = self.input.split_at(count);
            self.input = b;
            Ok(a)
        } else {
            Err(Error::DeserializeUnexpectedEnd)
        }
    }

    fn peek_major(&mut self) -> Result<u8> {
        if self.input.len() != 0 {
            let byte = self.input[0];
            Ok(byte >> 5)
        } else {
            Err(Error::DeserializeUnexpectedEnd)
        }
    }

    fn peek(&mut self) -> Result<u8> {
        if self.input.len() != 0 {
            Ok(self.input[0])
        } else {
            Err(Error::DeserializeUnexpectedEnd)
        }
    }

    fn consume(&mut self) -> Result<()> {
        if self.input.len() != 0 {
            self.input = &self.input[1..];
            Ok(())
        } else {
            Err(Error::DeserializeUnexpectedEnd)
        }
    }

    fn expect_major(&mut self, major: u8) -> Result<u8> {
        let byte = self.try_take_n(1)?[0];
        if major != (byte >> 5) {
            // logging::blocking::info!("expecting {}, got {} in byte {}", major, byte >> 5, byte).ok();
            // logging::blocking::info!("remaining data: {:?}", &self.input).ok();
            return Err(Error::DeserializeBadMajor);
        }
        Ok(byte & ((1 << 5) - 1))
    }

    // TODO: name something like "one-byte-integer"
    fn raw_deserialize_u8(&mut self, major: u8) -> Result<u8>
    {
        let additional = self.expect_major(major)?;

        match additional {
            byte @ 0..=23 => Ok(byte),
            24 => {
                match self.try_take_n(1)?[0] {
                    0..=23 => Err(Error::DeserializeNonMinimal),
                    byte => Ok(byte),
                }
            },
            _ => Err(Error::DeserializeBadU8),
        }
    }

    fn raw_deserialize_u16(&mut self, major: u8) -> Result<u16>
    {
        let number = self.raw_deserialize_u32(major)?;
        if number <= u16::max_value() as u32 {
            Ok(number as u16)
        } else {
            Err(Error::DeserializeBadU16)
        }
    }

    fn raw_deserialize_u32(&mut self, major: u8) -> Result<u32>
    {
        let additional = self.expect_major(major)?;

        match additional {
            byte @ 0..=23 => Ok(byte as u32),
            24 => {
                match self.try_take_n(1)?[0] {
                    0..=23 => Err(Error::DeserializeNonMinimal),
                    byte => Ok(byte as u32),
                }
            },
            25 => {
                let unsigned = u16::from_be_bytes(
                    self.try_take_n(2)?
                    .try_into().map_err(|_| Error::InexistentSliceToArrayError)?
                );
                match unsigned {
                    0..=255 => Err(Error::DeserializeNonMinimal),
                    unsigned => Ok(unsigned as u32),
                }
            },
            26 => {
                let unsigned = u32::from_be_bytes(
                    self.try_take_n(4)?
                    .try_into().map_err(|_| Error::InexistentSliceToArrayError)?
                );
                match unsigned {
                    0..=65535 => Err(Error::DeserializeNonMinimal),
                    unsigned => Ok(unsigned as u32),
                }
            },
            _ => Err(Error::DeserializeBadU32),
        }
    }

    // fn try_take_varint(&mut self) -> Result<usize> {
    //     for i in 0..VarintUsize::varint_usize_max() {
    //         let val = self.input.get(i).ok_or(Error::DeserializeUnexpectedEnd)?;
    //         if (val & 0x80) == 0 {
    //             let (a, b) = self.input.split_at(i + 1);
    //             self.input = b;
    //             let mut out = 0usize;
    //             for byte in a.iter().rev() {
    //                 out <<= 7;
    //                 out |= (byte & 0x7F) as usize;
    //             }
    //             return Ok(out);
    //         }
    //     }

    //     Err(Error::DeserializeBadVarint)
    // }
}

struct SeqAccess<'a, 'b: 'a> {
    deserializer: &'a mut Deserializer<'b>,
    len: usize,
}

impl<'a, 'b: 'a> serde::de::SeqAccess<'b> for SeqAccess<'a, 'b> {
    type Error = Error;

    fn next_element_seed<V>(&mut self, seed: V) -> Result<Option<V::Value>>
    where
        V: DeserializeSeed<'b>
    {
        if self.len > 0 {
            self.len -= 1;
            Ok(Some(seed.deserialize(&mut *self.deserializer)?))
        } else {
            Ok(None)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

struct MapAccess<'a, 'b: 'a> {
    deserializer: &'a mut Deserializer<'b>,
    len: usize,
}

impl<'a, 'b: 'a> serde::de::MapAccess<'b> for MapAccess<'a, 'b> {
    type Error = Error;

    fn next_key_seed<V>(&mut self, seed: V) -> Result<Option<V::Value>>
    where
        V: DeserializeSeed<'b>
    {
        if self.len > 0 {
            self.len -= 1;
            Ok(Some(seed.deserialize(&mut *self.deserializer)?))
        } else {
            Ok(None)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'b>,
    {
        seed.deserialize(&mut *self.deserializer)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

impl<'de, 'a> serde::de::VariantAccess<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<V::Value> {
        DeserializeSeed::deserialize(seed, self)
    }

    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
        serde::de::Deserializer::deserialize_tuple(self, len, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        serde::de::Deserializer::deserialize_tuple(self, fields.len(), visitor)
    }
}

impl<'de, 'a> serde::de::EnumAccess<'de> for &'a mut Deserializer<'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self)> {
        let discriminant = self.raw_deserialize_u32(0)?;
        // if discriminant > 0xFFFF_FFFF {
        //     return Err(Error::DeserializeBadEnum);
        // }
        let v = DeserializeSeed::deserialize(seed, discriminant.into_deserializer())?;
        Ok((v, self))
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    // ctapcbor does not support structures not known at compile time
    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // We wont ever support this.
        // If you need this, use `serde_cbor`.
        Err(Error::WontImplement)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let val = match self.try_take_n(1)?[0] {
            0xf4 => false,
            0xf5 => true,
            _ => return Err(Error::DeserializeBadBool),
        };
        visitor.visit_bool(val)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek_major()? {
            0 => {
                let raw_u8 = self.raw_deserialize_u8(0)?;
                if raw_u8 <= i8::max_value() as u8 {
                    visitor.visit_i8(raw_u8 as i8)
                } else {
                    Err(Error::DeserializeBadI8)
                }
            },
            1 => {
                let raw_u8 = self.raw_deserialize_u8(1)?;
                // if raw_u8 <= 1 + i8::max_value() as u8 {
                if raw_u8 <= 128 {
                    visitor.visit_i8(-1 - (raw_u8 as i16) as i8)
                } else {
                    Err(Error::DeserializeBadI8)
                }
            },
            _ => Err(Error::DeserializeBadI8),
        }
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek_major()? {
            0 => {
                let raw = self.raw_deserialize_u16(0)?;
                if raw <= i16::max_value() as u16 {
                    visitor.visit_i16(raw as i16)
                } else {
                    Err(Error::DeserializeBadI16)
                }
            },
            1 => {
                let raw = self.raw_deserialize_u16(1)?;
                if raw <= i16::max_value() as u16 {
                    visitor.visit_i16(-1 - (raw as i16))
                } else {
                    Err(Error::DeserializeBadI16)
                }
            },
            _ => Err(Error::DeserializeBadI16),
        }
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek_major()? {
            // TODO: figure out if this is BAAAAD for size or speed
            major @ 0..=1 => {
                let raw = self.raw_deserialize_u32(major)?;
                if raw <= i32::max_value() as u32 {
                    if major == 0 {
                        visitor.visit_i32(raw as i32)
                    } else {
                        visitor.visit_i32(-1 - (raw as i32))
                    }
                } else {
                    Err(Error::DeserializeBadI32)
                }
            },
            _ => Err(Error::DeserializeBadI16),
        }
    }

    fn deserialize_i64<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotYetImplemented)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw_deserialize_u8(0)?;
        visitor.visit_u8(raw)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw_deserialize_u16(0)?;
        visitor.visit_u16(raw)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw_deserialize_u32(0)?;
        visitor.visit_u32(raw)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let raw = self.raw_deserialize_u32(0)?;
        visitor.visit_u64(raw as u64)
    }

    fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotYetImplemented)
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::NotYetImplemented)
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // not sure, can this be implemented?
        // todo!("implement `deserialize_char`");
        Err(Error::NotYetImplemented)
        // let mut buf = [0u8; 4];
        // let bytes = self.try_take_n(4)?;
        // buf.copy_from_slice(bytes);
        // let integer = u32::from_le_bytes(buf);
        // visitor.visit_char(core::char::from_u32(integer).ok_or(Error::DeserializeBadChar)?)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // major type 2: "byte string"
        let length = self.raw_deserialize_u32(2)? as usize;
        let bytes: &'de [u8] = self.try_take_n(length)?;
        visitor.visit_borrowed_bytes(bytes)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // major type 3: "text string"
        let length = self.raw_deserialize_u32(3)? as usize;
        let bytes: &'de [u8] = self.try_take_n(length)?;
        let string_slice = core::str::from_utf8(bytes).map_err(|_| Error::DeserializeBadUtf8)?;
        visitor.visit_borrowed_str(string_slice)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.peek()? {
            0xf6 => {
                self.consume()?;
                visitor.visit_none()
            }
            _ => visitor.visit_some(self),
        }
    }

    // In Serde, unit means an anonymous value containing no data.
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek()? {
            0xf6 => {
                self.consume()?;
                visitor.visit_unit()
            }
            _ => Err(Error::DeserializeExpectedNull)
        }
    }

    // Unit struct means a named value containing no data.
    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // major type 4: "array"
        let len = self.raw_deserialize_u32(4)? as usize;

        visitor.visit_seq(SeqAccess {
            deserializer: self,
            len,
        })
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // major type 4: "array"
        let len = self.raw_deserialize_u32(4)? as usize;
        visitor.visit_seq(SeqAccess {
            deserializer: self,
            len,
        })
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // major type 5: "map"
        let len = self.raw_deserialize_u32(5)? as usize;

        visitor.visit_map(MapAccess {
            deserializer: self,
            len,
        })
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    // fn deserialize_enum<V>(
    //     self,
    //     _name: &'static str,
    //     _variants: &'static [&'static str],
    //     visitor: V,
    // ) -> Result<V::Value>
    // where
    //     V: Visitor<'de>,
    // {
    //     todo!("implement `deserialize_enum`");
    // }


    // fn parse_enum<V>(&mut self, mut len: usize, visitor: V) -> Result<V::Value>
    // where
    //     V: de::Visitor<'de>,
    // {
    //     self.recursion_checked(|de| {
    //         let value = visitor.visit_enum(VariantAccess {
    //             seq: SeqAccess { de, len: &mut len },
    //         })?;

    //         if len != 0 {
    //             Err(de.error(ErrorCode::TrailingData))
    //         } else {
    //             Ok(value)
    //         }
    //     })
    // }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek()? {
            0x82 => {
                self.consume()?;
                visitor.visit_enum(self)
                // // self.parse_enum(2, visitor)
                // let value = visitor.visit_enum(VariantAccess {
                //     seq: SeqAccess { self, len: &mut 2 },
                // })?;

                // if len != 0 {
                //     Err(de.error(ErrorCode::TrailingData))
                // } else {
                //     Ok(value)
                // }
            }
            // _ => Err(Error::DeserializeBadEnum),
            _ => visitor.visit_enum(self),
        }

        //     Some(byte @ 0x80..=0x9f) => {
        //         if !self.accept_legacy_enums {
        //             return Err(self.error(ErrorCode::WrongEnumFormat));
        //         }
        //         self.consume();
        //         match byte {
        //             0x80..=0x97 => self.parse_enum(byte as usize - 0x80, visitor),
        //             0x98 => {
        //                 let len = self.parse_u8()?;
        //                 self.parse_enum(len as usize, visitor)
        //             }
        //             0x99 => {
        //                 let len = self.parse_u16()?;
        //                 self.parse_enum(len as usize, visitor)
        //             }
        //             0x9a => {
        //                 let len = self.parse_u32()?;
        //                 self.parse_enum(len as usize, visitor)
        //             }
        //             0x9b => {
        //                 let len = self.parse_u64()?;
        //                 if len > usize::max_value() as u64 {
        //                     return Err(self.error(ErrorCode::LengthOutOfRange));
        //                 }
        //                 self.parse_enum(len as usize, visitor)
        //             }
        //             _ => Err(Error::DeserializeBadEnum),
        //             // 0x9c..=0x9e => Err(self.error(ErrorCode::UnassignedCode)),
        //             // 0x9f => self.parse_indefinite_enum(visitor),

        //             // _ => unreachable!(),
        //         }
        //     }
        //     _ => Err(Error::DeserializeBadEnum),
        //     // Some(0xa1) => {
        //     //     if !self.accept_standard_enums {
        //     //         return Err(self.error(ErrorCode::WrongEnumFormat));
        //     //     }
        //     //     self.consume();
        //     //     self.parse_enum_map(visitor)
        //     // }
        // }
        // println!("visiting enum");
        // let ret = visitor.visit_enum(self);
        // println!("visited enum");
        // ret
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // Ignore extra fields/options
        visitor.visit_none()
    }
}

// impl<'de, 'a> serde::de::VariantAccess<'de> for &'a mut Deserializer<'de> {
//     type Error = Error;

//     fn unit_variant(self) -> Result<()> {
//         Ok(())
//     }

//     fn newtype_variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<V::Value> {
//         DeserializeSeed::deserialize(seed, self)
//     }

//     fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
//         serde::de::Deserializer::deserialize_tuple(self, len, visitor)
//     }

//     fn struct_variant<V: Visitor<'de>>(
//         self,
//         fields: &'static [&'static str],
//         visitor: V,
//     ) -> Result<V::Value> {
//         serde::de::Deserializer::deserialize_tuple(self, fields.len(), visitor)
//     }
// }

// impl<'de, 'a> serde::de::EnumAccess<'de> for &'a mut Deserializer<'de> {
//     type Error = Error;
//     type Variant = Self;

//     fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self)> {
//         // let varint = self.try_take_varint()?;
//         // if varint > 0xFFFF_FFFF {
//         //     return Err(Error::DeserializeBadEnum);
//         // }
//         let varint = self.raw_deserialize_u32(0)?;
//         let v = DeserializeSeed::deserialize(seed, (varint as u32).into_deserializer())?;
//         Ok((v, self))
//     }
// }

// // // `MapAccess` is provided to the `Visitor` to give it the ability to iterate
// // // through entries of the map.
// // impl<'de, 'a> MapAccess<'de> for CommaSeparated<'a, 'de> {
// //     type Error = Error;

// //     fn next_key_seed<K>(&mut self, _seed: K) -> Result<Option<K::Value>>
// //     where
// //         K: DeserializeSeed<'de>,
// //     {
// //         // // Check if there are no more entries.
// //         // if self.de.peek_char()? == '}' {
// //         //     return Ok(None);
// //         // }
// //         // // Comma is required before every entry except the first.
// //         // if !self.first && self.de.next_char()? != ',' {
// //         //     return Err(Error::ExpectedMapComma);
// //         // }
// //         // self.first = false;
// //         // // Deserialize a map key.
// //         // seed.deserialize(&mut *self.de).map(Some)
// //         unimplemented!()
// //     }

// //     fn next_value_seed<V>(&mut self, _seed: V) -> Result<V::Value>
// //     where
// //         V: DeserializeSeed<'de>,
// //     {
// //         // // It doesn't make a difference whether the colon is parsed at the end
// //         // // of `next_key_seed` or at the beginning of `next_value_seed`. In this
// //         // // case the code is a bit simpler having it here.
// //         // if self.de.next_char()? != ':' {
// //         //     return Err(Error::ExpectedMapColon);
// //         // }
// //         // // Deserialize a map value.
// //         // seed.deserialize(&mut *self.de)
// //         unimplemented!()
// //     }
// // }

#[cfg(test)]
mod tests {
    // use super::*;
    use super::from_bytes;

    // use crate::serde::{cbor_serialize, cbor_serialize2, cbor_deserialize};
    // use crate::serde::{cbor_serialize, cbor_serialize_old, cbor_deserialize};
    use crate::serde::{cbor_serialize, cbor_deserialize};

    #[test]
    fn de_bool() {
        let mut buf = [0u8; 64];

        for boolean in [true, false].iter() {
            let _n = cbor_serialize(boolean, &mut buf).unwrap();
            let de: bool = from_bytes(&buf).unwrap();
            assert_eq!(de, *boolean);
        }
    }

    #[test]
    fn de_u8() {
        let mut buf = [0u8; 64];

        for number in 0..=255 {
            println!("testing {}", number);
            let _n = cbor_serialize(&number, &mut buf).unwrap();
            let de: u8 = from_bytes(&buf).unwrap();
            assert_eq!(de, number);
        }
    }

    #[test]
    fn de_i8() {
        let mut buf = [0u8; 64];

        for number in -128i8..=127 {
            println!("testing {}", number);
            let ser = cbor_serialize(&number, &mut buf).unwrap();
            println!("serialized: {:?}", ser);
            let de: i8 = cbor_deserialize(ser).unwrap();
            assert_eq!(de, number);
        }
    }


    #[test]
    fn de_u16() {
        let mut buf = [0u8; 64];

        for number in 0..=65535 {
            println!("testing {}", number);
            let _n = cbor_serialize(&number, &mut buf).unwrap();
            let de: u16 = from_bytes(&buf).unwrap();
            assert_eq!(de, number);
        }
    }

    #[test]
    fn de_i16() {
        let mut buf = [0u8; 64];

        for number in i16::min_value()..=i16::max_value() {
            println!("testing {}", number);
            let _n = cbor_serialize(&number, &mut buf).unwrap();
            let de: i16 = from_bytes(&buf).unwrap();
            assert_eq!(de, number);
        }
    }

    #[test]
    fn de_u32() {
        let mut buf = [0u8; 64];

        for number in 0..=3*(u16::max_value() as u32) {
            println!("testing {}", number);
            let _n = cbor_serialize(&number, &mut buf).unwrap();
            let de: u32 = from_bytes(&buf).unwrap();
            assert_eq!(de, number);
        }

        for number in (u32::max_value() - u16::max_value() as u32)..=u32::max_value() {
            println!("testing {}", number);
            let _n = cbor_serialize(&number, &mut buf).unwrap();
            let de: u32 = from_bytes(&buf).unwrap();
            assert_eq!(de, number);
        }
    }

    #[test]
    fn de_i32() {
        let mut buf = [0u8; 64];

        let number: i32 = -98304;
        let ser = cbor_serialize(&number, &mut buf).unwrap();
        println!("serialized number: {:?} of {}", ser, i16::min_value());
        let de: i32 = from_bytes(ser).unwrap();
        assert_eq!(de, number);

        for number in (3*i16::min_value() as i32)..=3*(i16::max_value() as i32) {
            println!("testing {}", number);
            let ser = cbor_serialize(&number, &mut buf).unwrap();
            let de: i32 = from_bytes(ser).unwrap();
            assert_eq!(de, number);
        }

        for number in (i32::max_value() - i16::max_value() as i32)..=i32::max_value() {
            println!("testing {}", number);
            let ser = cbor_serialize(&number, &mut buf).unwrap();
            let de: i32 = from_bytes(ser).unwrap();
            assert_eq!(de, number);
        }

        for number in i32::min_value()..=(i32::min_value() - i16::min_value() as i32) {
            println!("testing {}", number);
            let ser = cbor_serialize(&number, &mut buf).unwrap();
            let de: i32 = from_bytes(ser).unwrap();
            assert_eq!(de, number);
        }
    }

    #[test]
    fn de_bytes() {
        use heapless::consts::U64;

        let mut buf = [0u8; 64];

        let slice = b"thank you postcard!";
        let bytes = heapless::ByteBuf::<U64>::from_slice(slice).unwrap();
        let ser = cbor_serialize(&bytes, &mut buf).unwrap();
        println!("serialized bytes = {:?}", ser);
        let de: heapless::ByteBuf::<U64> = from_bytes(&buf).unwrap();
        println!("deserialized bytes = {:?}", &de);
        assert_eq!(&de, slice);
    }

    #[test]
    fn de_str() {
        use heapless::consts::U64;

        let mut buf = [0u8; 64];

        let string_slice = "thank you postcard, for blazing the path 🐝";
        let mut string = heapless::String::<U64>::new();
        string.push_str(string_slice).unwrap();
        let _n = cbor_serialize(&string, &mut buf);
        let de: heapless::String<U64> = from_bytes(&buf).unwrap();
        assert_eq!(de, string_slice);
    }

    #[test]
    fn de_struct() {
        use crate::ctap2::get_info::CtapOptions;
        // rk: bool,
        // up: bool,
        // #[serde(skip_serializing_if = "Option::is_none")]
        // uv: Option<bool>,
        // plat: bool,
        // #[serde(skip_serializing_if = "Option::is_none")]
        // client_pin: Option<bool>,
        // #[serde(skip_serializing_if = "Option::is_none")]
        // cred_protect: Option<bool>,

        let options = CtapOptions {
            rk: false,
            up: true,
            uv: None,
            plat: Some(false),
            client_pin: Some(true),
        };

        let mut buf = [0u8; 64];

        let _n = cbor_serialize(&options, &mut buf);
        let de: CtapOptions = from_bytes(&buf).unwrap();
        assert_eq!(de, options);
    }

    #[test]
    fn de_credential_id() {
        use heapless::{ByteBuf, consts::{U32, U64}};
        use serde_indexed::{DeserializeIndexed, SerializeIndexed};
        #[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
        pub struct CredentialInner {
            pub user_id: ByteBuf<U64>,
            pub alg: i8,
            pub seed: ByteBuf<U32>,
        }

        let input = b"\xa3\x00Gnickray\x01&\x02X @7\xbf\xa6\x98j\xb9\x0e8nB\x92\xd8\xf2\x1bK\xef\x92\xe87\xfe2`\x92%\xff\x98jR\xd1\xc8\xc1";

        let _credential_inner: CredentialInner = from_bytes(input).unwrap();
    }

    #[test]
    fn de_enum() {

        let mut buf = [0u8; 64];
        let e = Some(3);
        let ser = cbor_serialize(&e, &mut buf).unwrap();
        println!("ser(Some(3)) = {:?}", ser);
        let de: Option<u8> = cbor_deserialize(ser).unwrap();
        assert_eq!(de, e);
        let e: Option<u8> = None;
        println!("ser({:?}) = {:x?}", &e, cbor_serialize(&e, &mut buf).unwrap());

        // let mut buf = [0u8; 64];
        // let _n = cbor_serialize(&None, &mut buf).unwrap();
        // println!("ser(e) = {:?}", &buf[.._n]);

        // use serde_indexed::{DeserializeIndexed, SerializeIndexed};
        use serde::{Deserialize, Serialize};
        #[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
        pub enum Enum {
            Alpha(u8),
            // Beta((i32, u32)),
            Beta(i32),
        }

        let mut buf = [0u8; 64];

        // let e = Enum::Beta((-42, 7));
        let e = Enum::Beta(-42);
        let ser = cbor_serialize(&e, &mut buf).unwrap();
        println!("ser({:?}) = {:?}", &e, ser);
        let de: Enum = cbor_deserialize(ser).unwrap();
        assert_eq!(de, e);

        #[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
        pub enum SimpleEnum {
            // Alpha(u8),
            Alpha(u8),
            Beta,
        }

        let e = SimpleEnum::Alpha(7);
        let ser = cbor_serialize(&e, &mut buf).unwrap();
        println!("ser({:?}) = {:?}", &e, ser);
        let de: SimpleEnum = cbor_deserialize(ser).unwrap();
        assert_eq!(de, e);

        let e = SimpleEnum::Beta;
        let ser = cbor_serialize(&e, &mut buf).unwrap();
        println!("ser({:?}) = {:?}", &e, ser);
        let de: SimpleEnum = cbor_deserialize(ser).unwrap();
        assert_eq!(de, e);
    }

    #[test]
    fn fuzzer_things() {
        let data: [u8; 2] = [160, 96];
        type T = crate::webauthn::PublicKeyCredentialUserEntity;
        cbor_deserialize::<T>(&data).ok();
    }

    // #[test]
    // fn piv_persistent_state() {
    //     let data = b"\xa6dkeys\xa2rauthentication_keyP<\xc1\xaa\x8c\xc3\xfav4\x88\xbc\xdb\x9fe\x81\xa7nnmanagement_keyP\x8c\x16\"\xed\x0f\xce\x9c\xac^\xf1;\xd0r\xea\xc9\xcbx\x1aconsecutive_pin_mismatches\x00x\x1aconsecutive_puk_mismatches\x00cpin\xa1jpadded_pin\x88\x181\x182\x183\x181\x182\x183\x18\xff\x18\xffcpuk\xa1jpadded_pin\x88\x181\x182\x183\x181\x182\x183\x18\xff\x18\xffitimestamp\x00";

    //     cbor_deserialize::<T>(&data).ok();
    // }
}
