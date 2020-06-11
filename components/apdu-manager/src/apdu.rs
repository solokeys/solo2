
use crate::traits::Error;

pub struct Apdu<'a> {
    pub cla: u8,
    pub ins: u8,
    pub p1: u8,
    pub p2: u8,
    pub lc: u16,
    pub le: u32,
    pub case: u8,
    pub offset: usize,
    pub buffer: &'a mut[u8],
}

impl<'a> Apdu<'a> {

    /// Apdu decoding made much easier due to Oleg Moiseenko's clear C implementation.
    /// https://github.com/solokeys/solo/blob/master/fido2/apdu.c#L12
    pub fn new (bin: &mut [u8], apdu_len: usize) -> Result<Apdu, Error> {

        if apdu_len < 4 {
            return Err(Error::SwWrongLength);
        }

        // handle the 8 different ways an apdu can specify length...
        let (lc,le,case) = if apdu_len == 4 {
            // Case 1 ()
            (0u16, 0u32, 1)

        } else if apdu_len == 5 {

            // Case 2 (Le0)
            (0u16, bin[4] as u32, 2)

        } else if apdu_len == (bin[4] as usize + 5) {

            // Case 3 (Lc0 + data)
            (
                if bin[4] == 0 { 256u16 } else { bin[4] as u16 }, 
                0u32, 
                3
            )

        } else if apdu_len == (bin[4] as usize + 5 + 1) {

            // Case 3 (Lc0 + data + Le0)
            (
                if bin[4] == 0 { 256u16 } else { bin[4] as u16 }, 
                if *bin.last().unwrap() == 0 { 0x10000u32 } else { *bin.last().unwrap() as u32 }, 
                4
            )
        }

        else if apdu_len >= 7 && bin[4] == 0 {
            let extended_len = ((bin[5] as u16) << 8) + bin[6] as u16;

            if apdu_len == 7 {
                // Case 2 extended (Le0 Le1)
                (0u16, extended_len as u32, 0x12)
            }
            else if (apdu_len - 7) == extended_len as usize {

                // Case 3 extended (Lc0 Lc1 Lc2 data)
                (extended_len as u16, 0u32, 0x13)
            }
            else if (apdu_len > 8) && (apdu_len - 7 - 2) == extended_len as usize 
            || (apdu_len > 9) &&(apdu_len - 7 - 3) == extended_len as usize
            {
                // Case 4 extended (Lc0 Lc1 Lc2 data Le0 Le1 [Le2])
                let le_raw = ((bin[apdu_len-2] as u32) << 8) | bin[apdu_len - 1] as u32;
                (
                    extended_len as u16, 
                    if le_raw == 0 { 0x10000u32 } else { le_raw }, 
                    0x14
                )
            }
            else {
                return Err(Error::SwWrongLength);
            }
            


        }

        else {
            return Err(Error::SwWrongLength);
        };

        let (cla, ins, p1, p2) = (bin[0],bin[1],bin[2],bin[3],);

        let offset = if lc == 0 {
            4
        } else if case < 0x10 {
            5
        } else {
            7
        };

        Ok(
            Apdu{
                cla: cla,
                ins: ins,
                p1: p1,
                p2: p2,
                lc: lc,
                le: le,
                case: case,
                offset: offset,
                buffer: bin,
            }
        )
    }

    pub fn new_fixed (bin: &mut [u8]) -> Result<Apdu, Error> {
        let l = bin.len();
        Self::new(bin, l)
    }
}
