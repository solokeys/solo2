/// Extract of relevant parts of `aligned` library,
/// without its multi-generic-array dependency tree.

use core::ops;

pub trait Alignment {}

/// 4-byte alignment
#[repr(align(4))]
pub struct A4;

impl Alignment for A4 {}

/// A newtype with alignment of at least `A` bytes
#[repr(C)]
pub struct Aligned<A, T>
where
    T: ?Sized,
{
    _alignment: [A; 0],
    value: T,
}

/// Changes the alignment of `value` to be at least `A` bytes
#[allow(non_snake_case)]
pub const fn Aligned<A, T>(value: T) -> Aligned<A, T> {
    Aligned {
        _alignment: [],
        value,
    }
}

impl<A, T> ops::Deref for Aligned<A, T>
where
    A: Alignment,
    T: ?Sized,
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<A, T> ops::DerefMut for Aligned<A, T>
where
    A: Alignment,
    T: ?Sized,
{
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

