use crate::std;

#[repr(align(128))]
struct ArcInner<T> {
    count: crate::Align128<std::AtomicIsize>,
    data: T,
}

pub struct Arc<T> {
    inner: std::NonNull<ArcInner<T>>,
    phantom: std::PhantomData<ArcInner<T>>,
}

impl<T> Arc<T> {
    pub fn raw(data: T) -> *const () {
        let uninit = unsafe {
            std::alloc(std::Layout::new::<ArcInner<T>>()) as *mut ArcInner<T>
        };

        unsafe {
            std::ptr::write(
                uninit,
                ArcInner {
                    count: crate::Align128(std::AtomicIsize::new(0)),
                    data,
                },
            );
        }

        uninit as _
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
            inner: std::NonNull::new_unchecked(ptr as _),
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

        self.inner.as_ref().count.fetch_add(count, std::Relaxed);
    }
}

impl<T> Arc<T>
where
    T: std::Copy,
{
    pub unsafe fn data_from_raw(ptr: *const ()) -> T {
        (*(ptr as *const ArcInner<T>)).data
    }
}

impl<T> std::Drop for Arc<T> {
    /// Decrements the read count of the inner `ArcInner`. If the count reaches 0,
    /// the boxed `ArcInner` is dropped.
    fn drop(&mut self) {
        let prev_count =
            unsafe { self.inner.as_ref().count.fetch_sub(1, std::Relaxed) };

        if 1 == prev_count {
            // A count of **exactly** 0 implies exclusive access to the boxed
            // `ArcInner` and ensures safe construction of the `Box` to drop it.
            unsafe {
                std::dealloc(
                    self.inner.as_ptr() as _,
                    std::Layout::new::<ArcInner<T>>(),
                );
            }
        }
    }
}
