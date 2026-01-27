use littlefs2::{
    consts,
    fs::{File, Filesystem},
    io::{Error, Result, Write},
    ram_storage, driver,
};

ram_storage!(
    name=OtherRamStorage,
    backend=OtherRam,
    trait=driver::Storage,
    erase_value=0xff,
    read_size=1,
    write_size=32,
    cache_size_ty=consts::U32,
    block_size=256,
    block_count=512,
    lookaheadwords_size_ty=consts::U1,
    filename_max_plus_one_ty=consts::U256,
    path_max_plus_one_ty=consts::U256,
    result=Result,
);

ram_storage!(
    name=RamStorage,
    backend=Ram,
    trait=driver::Storage,
    erase_value=0xff,
    read_size=20*5,
    write_size=20*7,
    cache_size_ty=consts::U700,
    block_size=20*35,
    block_count=32,
    lookaheadwords_size_ty=consts::U1,
    filename_max_plus_one_ty=consts::U256,
    path_max_plus_one_ty=consts::U256,
    result=Result,
);

fn main() {
    let mut ram = Ram::default();
    let mut storage = RamStorage::new(&mut ram);
    let mut alloc = Filesystem::allocate();
    Filesystem::format(&mut storage).unwrap();
    let mut fs = Filesystem::mount(&mut alloc, &mut storage).unwrap();

    let mut other_ram = OtherRam::default();
    let mut other_storage = OtherRamStorage::new(&mut other_ram);
    let mut alloc = Filesystem::allocate();
    assert!(Filesystem::format(&mut other_storage).is_ok());
    let other_fs = Filesystem::mount(&mut alloc, &mut other_storage).unwrap();

    let mut alloc = File::allocate();
    // file does not exist yet, can't open for reading
    assert_eq!(
        File::open("/test_open.txt", &mut alloc, &mut fs, &mut storage)
            .map(drop)
            .unwrap_err(), // "real" contains_err is experimental
        Error::NoSuchEntry
    );

    fs.create_dir("/tmp", &mut storage).unwrap();

    // TODO: make previous allocation reusable
    let mut alloc = File::allocate();
    // can create new files
    let mut file = File::create("/tmp/test_open.txt", &mut alloc, &mut fs, &mut storage).unwrap();
    // can write to files
    assert!(file.write(&mut fs, &mut storage, &[0u8, 1, 2]).unwrap() == 3);
    // won't work: "expected `RamStorage`, found `RamStorageNormal`.
    file.sync(&mut other_fs, &mut other_storage).unwrap();
}
