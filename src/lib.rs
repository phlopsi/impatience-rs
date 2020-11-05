#![no_implicit_prelude]
#![forbid(dead_code)]
#![forbid(unused_unsafe)]

mod arc;
mod arc_handle;
pub mod spsc;
mod std;

use crate::arc::Arc;
use crate::arc_handle::ArcHandle;

pub struct AtomicCell<T>
where
    T: std::Copy,
{
    handle: crate::ArcHandle<T>,
    phantom: std::PhantomData<std::Mutex<T>>,
}

// TODO: Auto Trait Implementations

impl<T> AtomicCell<T>
where
    T: std::Copy,
{
    pub fn new(value: T) -> Self {
        Self {
            handle: crate::ArcHandle::new(value),
            phantom: std::PhantomData,
        }
    }

    pub fn set(&self, value: T) {
        self.handle.swap(&mut crate::ArcHandle::new(value));
    }

    pub fn get(&self) -> T {
        self.handle.get()
    }
}
