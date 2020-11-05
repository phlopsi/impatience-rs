use crate::std;

#[repr(align(128))]
struct RawArc<T> {
    count: crate::Align128<std::AtomicIsize>,
    data: T,
}

pub struct Arc<T> {
    ptr: std::NonNull<RawArc<T>>,
    phantom: std::PhantomData<RawArc<T>>,
}

impl<T> Arc<T> {
    pub fn raw(data: T) -> *const () {
        std::Box::into_raw(std::Box::new(RawArc {
            count: crate::Align128(std::AtomicIsize::new(0)),
            data,
        })) as _
    }

    /// Constructs an `Arc<T>` from a raw pointer.
    ///
    /// The raw pointer must have been previously returned by a call to
    /// [`Arc<U>::raw`][raw] where `U` must have the same size and
    /// alignment as `T`. This is trivially true if `U` is `T`.
    /// Note that if `U` is not `T` but has the same size and alignment, this is
    /// basically like transmuting references of different types. See
    /// `std::mem::transmute` for more information on what
    /// restrictions apply in this case.
    ///
    /// The user of `from_raw` has to make sure a specific value of `T` is only
    /// dropped once.
    ///
    /// This function is unsafe because improper use may lead to memory unsafety,
    /// even if the returned `Arc<T>` is never accessed.
    ///
    /// [raw]: struct.Arc.html#method.raw
    pub unsafe fn from_raw(ptr: *const ()) -> Self {
        Self {
            ptr: std::NonNull::new_unchecked(ptr as _),
            phantom: std::PhantomData,
        }
    }

    /// Initializes the reference count.
    ///
    /// Used for synchronization purposes and freeing the underlying memory.
    ///
    /// # Safety
    ///
    /// This method **must not** be called more than once. The provided count
    /// **must** be at least 1.
    pub unsafe fn init_count(&self, count: isize) {
        {
            use std::panic;
            std::debug_assert!(count >= 1);
        }

        self.ptr.as_ref().count.fetch_add(count, std::SeqCst);
    }
}

impl<T> Arc<T>
where
    T: std::Copy,
{
    pub unsafe fn data_from_raw(ptr: *const ()) -> T {
        (*(ptr as *const RawArc<T>)).data
    }
}

impl<T> std::Drop for Arc<T> {
    /// Decrements the read count of the inner `RawArc`. If the count reaches 0,
    /// the boxed `RawArc` is dropped.
    fn drop(&mut self) {
        let count = unsafe {
            self.ptr
                .as_ref()
                .count
                .fetch_sub(1, std::SeqCst)
                .checked_sub(1)
                .unwrap_or_else(|| std::unreachable_unchecked())
        };

        if 0 == count {
            // A count of **exactly** 0 implies exclusive access to the boxed
            // `RawArc` and ensures safe construction of the `Box` to drop it.
            std::drop(unsafe { std::Box::from_raw(self.ptr.as_ptr()) });
        }
    }
}
