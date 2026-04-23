use serde_cbor::Value;
use std::collections::BTreeMap;

pub type CborMap = BTreeMap<Value, Value>;

pub fn int_key(key: i128) -> Value {
    Value::Integer(key)
}

pub fn text(value: &str) -> Value {
    Value::Text(value.into())
}

pub fn bytes<const N: usize>(value: [u8; N]) -> Value {
    Value::Bytes(value.to_vec())
}

pub fn bytes_vec(value: Vec<u8>) -> Value {
    Value::Bytes(value)
}

pub fn map(entries: impl IntoIterator<Item = (Value, Value)>) -> Value {
    Value::Map(entries.into_iter().collect())
}

pub fn array(entries: impl IntoIterator<Item = Value>) -> Value {
    Value::Array(entries.into_iter().collect())
}

pub fn encode(value: &Value) -> Vec<u8> {
    serde_cbor::to_vec(value).expect("serialize raw CTAP request")
}
