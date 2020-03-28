use serde::{Serialize, Deserialize};
use ctap_types::serde::{cbor_serialize, cbor_deserialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Example {
    foo: Foo,
    payload: u8,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Foo {
    x: u8,
    color: Color,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Color {
    Red,
    Blue,
    Yellow(u8),
}

const EXAMPLE: Example = Example {
    foo: Foo {
        x: 0xAA,
        color: Color::Yellow(40),
    },
    payload: 0xCC,
};

#[test]
fn test() {
    let mut slice = [0u8; 64];
    let roundtrip = cbor_deserialize(cbor_serialize(&EXAMPLE, &mut slice).unwrap()).unwrap();
    assert_eq!(EXAMPLE, roundtrip);
}
