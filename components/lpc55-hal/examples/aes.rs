#![no_main]
#![no_std]

///
/// Compare also with: https://github.com/Ko-/aes-armcortexm
///
extern crate panic_semihosting;
use cortex_m_rt::entry;

use core::convert::TryInto;

#[allow(unused_imports)]
use hal::prelude::*;
#[allow(unused_imports)]
use lpc55_hal as hal;

use aes::cipher::NewBlockCipher;
use hal::traits::cipher::{BlockDecrypt, BlockEncrypt};

use generic_array::GenericArray;

use cortex_m_semihosting::{dbg, hprintln};

#[entry]
fn main() -> ! {
    let dp = hal::raw::Peripherals::take().unwrap();
    let mut syscon = hal::Syscon::from(dp.SYSCON);
    let mut hashcrypt = hal::Hashcrypt::from(dp.HASHCRYPT).enabled(&mut syscon);

    let raw_key = [0u8; 32];
    let key = GenericArray::from_slice(&raw_key);

    let raw_block = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let block = GenericArray::clone_from_slice(&raw_block);

    //
    // via software
    //
    let mut sw_block = block.clone();
    let cipher = aes::Aes256::new(&key);

    let (sw_cyc_enc, _) = hal::count_cycles(|| {
        cipher.encrypt_block(&mut sw_block);
    });
    hprintln!("encrypting with aes-soft took {} cycles", sw_cyc_enc).unwrap();

    let sw_encrypted_block: [u8; 16] = sw_block[..].try_into().unwrap();

    let (sw_cyc_dec, _) = hal::count_cycles(|| {
        cipher.decrypt_block(&mut sw_block);
    });
    hprintln!("decrypting with aes-soft took {} cycles", sw_cyc_dec).unwrap();

    // check sw decrypt⚬encrypt = id
    assert_eq!(sw_block, block);

    //
    // via hardware
    //
    let mut hw_block = block.clone();
    let cipher = hashcrypt.aes256(&raw_key);

    cipher.prime_for_encryption();
    dbg!("running hw encrypt");
    let (hw_cyc_enc, _) = hal::count_cycles(|| {
        cipher.encrypt_block(&mut hw_block);
    });
    hprintln!("encrypting with hashcrypt took {} cycles", hw_cyc_enc).unwrap();
    hprintln!("speedup: {}x", sw_cyc_enc / hw_cyc_enc).unwrap();
    // dbg!(hw_block);

    let hw_encrypted_block: [u8; 16] = hw_block.as_slice().try_into().unwrap();

    // check HW implementation works properly, by comparing against known good SW
    assert_eq!(sw_encrypted_block, hw_encrypted_block);

    cipher.prime_for_decryption();
    dbg!("running hw decrypt");
    let (hw_cyc_dec, _) = hal::count_cycles(|| {
        cipher.decrypt_block(&mut hw_block);
    });
    hprintln!("decrypting with hashcrypt took {} cycles", hw_cyc_dec).unwrap();
    hprintln!("speedup: {}x", sw_cyc_dec / hw_cyc_dec).unwrap();

    // check hw decrypt⚬encrypt = id
    assert_eq!(hw_block, block);


    // // Finally, PUF key
    // let cipher = hashcrypt.puf_aes();
    // dbg!(hw_block);
    // dbg!(cipher.decrypt_block(&mut hw_block));
    // dbg!(hw_block);

    // DONE
    dbg!("all done");
    loop { continue; }
}
