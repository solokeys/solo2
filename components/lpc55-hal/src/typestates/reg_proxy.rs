use core::{
    marker::PhantomData,
    ops::Deref,
};

use crate::traits::reg_proxy::{
    Reg,
    RegCluster,
};

pub struct RegProxy<T: Reg> {
    _marker: PhantomData<*const T>,
}

impl<T: Reg> RegProxy<T> {
    /// Create a new proxy object
    #[allow(dead_code)]
    pub fn new() -> Self {
        RegProxy {
            _marker: PhantomData,
        }
    }
}

unsafe impl<T> Send for RegProxy<T> where T: Reg {}

impl<T: Reg> Deref for RegProxy<T> {
    type Target = T::Target;

    fn deref(&self) -> &Self::Target {
        // As long as `T` upholds the safety restrictions laid out in the
        // documentation of `Reg`, this should be safe. The pointer is valid for
        // the duration of the program. That means:
        // 1. It can always be dereferenced, so casting to a reference is safe.
        // 2. It is essentially `'static`, so casting to any lifetime is safe.
        unsafe { &*T::get() }
    }
}

// For clusters, e.g. GPIO's set, clr and dirset
pub struct RegClusterProxy<T: RegCluster> {
    _marker: PhantomData<*const [T]>,
}

impl<T: RegCluster> RegClusterProxy<T> {
    pub fn new() -> Self {
        RegClusterProxy {
            _marker: PhantomData,
        }
    }
}

unsafe impl<T> Send for RegClusterProxy<T> where T: RegCluster {}

impl<T: RegCluster> Deref for RegClusterProxy<T> {
    type Target = [T::Target];

    fn deref(&self) -> &Self::Target {
        unsafe { &*T::get() }
    }
}
