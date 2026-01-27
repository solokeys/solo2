use littlefs2::{
    consts,
    driver,
    fs::Filesystem,
    io::Result,
    path::Path,
    ram_storage,
};

use serde::{Deserialize,Serialize};
use ssmarshal::{deserialize, serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Default/*, Desse, DesseSized*/)]
struct Entity {
	z: [u8; 16],
    // x: u32,
    x: f32,
    y: u64,
}

ram_storage!(tiny);

#[test]
fn main() {
    let mut ram = Ram::default();
    let mut storage = RamStorage::new(&mut ram);
    Filesystem::format(&mut storage).unwrap();

    let mut alloc = Filesystem::allocate();
    let fs = Filesystem::mount(&mut alloc, &mut storage).unwrap();

    let entity = Entity::default();

    let mut buf = [0u8; core::mem::size_of::<Entity>()];
    assert_eq!(buf.len(), 32);
    let size = serialize(&mut buf, &entity).unwrap();
    assert_eq!(size, 28);

    fs.write(Path::from_bytes_with_nul(b"entity.bin\0").unwrap(), &buf[..size]).unwrap();
    fs.open_file_and_then(Path::from_bytes_with_nul(b"entity.bin\0").unwrap(), |file| {
        file.read(&mut buf)
    }).unwrap();
    // let mut alloc = File::allocate();
    // let mut file = File::create("entity.bin", &mut alloc, &mut fs, &mut storage).unwrap();
    // file.write(&mut fs, &mut storage, &buf[..size]).unwrap();
    // file.sync(&mut fs, &mut storage).unwrap();

    // let mut file = File::open("entity.bin", &mut alloc, &mut fs, &mut storage).unwrap();
    // file.read(&mut fs, &mut storage, &mut buf).unwrap();
    let read_entity: Entity = deserialize(&buf).unwrap().0;

    assert_eq!(read_entity, entity);
}
