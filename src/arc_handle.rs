use crate::std;
use crate::Arc;

const BITS_PER_BYTE: usize = 8;
const LOG_2_128: usize = 7;
const PTR_BIT_MASK: usize = usize::MAX >> LOG_2_128;
const USIZE_BITS: usize = std::size_of::<usize>() * BITS_PER_BYTE;
const DATA_BIT_SHIFT: usize = USIZE_BITS - LOG_2_128;

fn raw_arc_handle_from_ptr(ptr: *const ()) -> usize {
    ptr as usize >> LOG_2_128
}

fn raw_arc_handle_ptr(value: usize) -> *const () {
    (value << LOG_2_128) as _
}

fn raw_arc_handle_count(value: usize) -> isize {
    (value >> DATA_BIT_SHIFT) as _
}

fn raw_arc_handle_inc_count(value: usize) -> usize {
    fn checked_inc(value: usize) -> std::Option<usize> {
        if value < 127 {
            std::Some(value.checked_add(1).unwrap_or_else(|| {
                // SAFETY: `value` has been confirmed to be less than 127 (the maximum value representable with a 7 bit unsigned integer), i.e. incrementing it by 1 cannot cause an overflow or invalid value.
                unsafe { std::unreachable_unchecked() }
            }))
        } else {
            std::None
        }
    }

    (checked_inc(value >> DATA_BIT_SHIFT).unwrap() << DATA_BIT_SHIFT)
        | (value & PTR_BIT_MASK)
}

fn raw_arc_handle_dec_count(value: usize) -> usize {
    ((value >> DATA_BIT_SHIFT).checked_sub(1).unwrap() << DATA_BIT_SHIFT)
        | (value & PTR_BIT_MASK)
}

// Loading the raw handle does not permit dereferencing the contained raw
// pointer. The memory may have already been deallocated, if a context
// switch occured before the data has been read. This would lead to a
// use-after-free bug and thus undefined behavior.
//
// Trying to increment a separate atomic counter to signal a read before
// loading the raw pointer does work. However, it also prevents any attempts
// at mutating the raw handle, thus locking the raw handle similar to a
// `RwLock`. It is not possible to atomically swap out the handle and the
// counter in one go with 2 atomic fields.
//
// To achieve guaranteed atomicity, the counter and pointer need to reside
// within the same atomic field. Due to atomics requiring exclusive access
// to the cache line they're in, aligning them to the cache line is a
// natural choice to prevent cache line bouncing. The maximum known cache
// line size known to me is 128 bytes, thus for simplicity's sake the raw
// pointer is always aligned to 128 bytes. That means, it is sound to
// logically shift the address by 7 bits to the right and left (in that
// order) without loss of information. 7 bits can be used to encode 128
// unique states. For our purpose the bits are used to encode the number of
// current read accesses. Should the raw handle be replaced, the mutator
// will receive the raw pointer + the number of readers at the time of the
// swap. The data structure containing the data also contains a counter.
// This counter is used to determine when the memory can be deallocated.
//
// Each reader tries to reduce the counter in the raw handle once they're
// finished with their task of reading the pointed-at data. If the raw
// handle changed, the reader will look at what part has changed. If only
// the count has changed, the reader will try to update it with a new value,
// otherwise the pointer was changed. In the latter case, the reader
// dereferences the pointed-at value, again and decrements the counter to
// signal, that the reader finished their task. The memory will be
// deallocated once all readers and writers finish their task and promise to
// not dereference the pointer, again. This happens when the counter reaches
// 0 a second time. The writer increases the counter by the number of
// registered readers + 1 (for the writer). For example, if 100 readers were
// registered when the writer obtained the raw handle, the counter will be
// increased by 101 by the writer and each reader + the writer decreases the
// counter by 1, thus reaching 0, eventually and deallocating the memory.
pub struct ArcHandle<T>
where
    T: std::Copy,
{
    handle: std::AtomicUsize,
    phantom: std::PhantomData<Arc<T>>,
}

impl<T> ArcHandle<T>
where
    T: std::Copy,
{
    pub fn new(data: T) -> Self {
        let ptr = Arc::raw(data);
        let raw_handle = raw_arc_handle_from_ptr(ptr);
        let handle = std::AtomicUsize::new(raw_handle);

        Self {
            handle,
            phantom: std::PhantomData,
        }
    }

    pub fn swap(&self, other: &mut Self) {
        let raw_handle_mut = other.handle.get_mut();
        *raw_handle_mut = self.handle.swap(*raw_handle_mut, std::SeqCst);
    }

    pub fn get(&self) -> T {
        let mut raw_handle = self.handle.load(std::SeqCst);
        let mut raw_handle_new;

        // Obtain a raw handle and update the read count
        loop {
            raw_handle_new = raw_arc_handle_inc_count(raw_handle);

            let result = self.handle.compare_exchange_weak(
                raw_handle,
                raw_handle_new,
                std::SeqCst,
                std::SeqCst,
            );

            match result {
                std::Ok(_) => {
                    break;
                }
                std::Err(raw_handle_current) => {
                    raw_handle = raw_handle_current;
                }
            }
        }

        let raw_arc_ptr = raw_arc_handle_ptr(raw_handle_new);

        // SAFETY: The read count of the atomic variable has been incremented
        //   and the memory is not freed before the internal counter has been
        //   reduced to 0. See: `Drop::drop` for `Arc`
        let data = unsafe { Arc::data_from_raw(raw_arc_ptr) };

        loop {
            let result = self.handle.compare_exchange(
                raw_handle_new,
                raw_handle,
                std::SeqCst,
                std::SeqCst,
            );

            match result {
                std::Ok(_) => {
                    return data;
                }
                std::Err(raw_handle_current) => {
                    let raw_handle_current = raw_handle_current;

                    if raw_arc_handle_ptr(raw_handle_current) == raw_arc_ptr {
                        // The raw pointer remains the same. That's why we try
                        // to decrement the count with updated values, again.
                        raw_handle_new = raw_handle_current;
                        raw_handle =
                            raw_arc_handle_dec_count(raw_handle_current);
                    } else {
                        // The raw pointer has been swapped out, i.e. the read
                        // count embedded in the raw handle will be used to
                        // increase the inner Arc's counter. We have to
                        // decrement the Arc's counter to ensure the Arc will
                        // eventually free the memory.
                        //
                        // SAFETY: The read count of the atomic variable hasn't
                        //   been decremented, i.e. the Arc cannot be freed,
                        //   yet. See: `Drop::drop` for `Arc`
                        std::drop(unsafe { Arc::<T>::from_raw(raw_arc_ptr) });

                        return data;
                    }
                }
            }
        }
    }
}

impl<T> std::Drop for ArcHandle<T>
where
    T: std::Copy,
{
    fn drop(&mut self) {
        unsafe {
            let raw_handle = *self.handle.get_mut();

            // SAFETY: Arc can and must be constructed in two ways only:
            // 1) By `ArcHandle::get`, if and only if the read count from the raw
            //    Arc handle could not be decremented, because it was swapped out.
            // 2) Unconditionally inside this method.
            //
            // This method assumes correct mutation of the reference count.
            let arc = Arc::<T>::from_raw(raw_arc_handle_ptr(raw_handle));

            // It's important to add 1 to the reader count before initializing
            // the count and then dropping the `Arc<T>`. Not doing so would leak
            // memory in the case no one has ever read the value, i.e. the drop
            // code is never run.
            //
            // This addition and following drop could be done conditionally,
            // but it's unclear how the branch will affect the instruction cache
            // and thus performance, since it is unpredictable which branch is
            // taken. On the other hand, dropping requires decrementing the
            // reference count, which has synchronization overhead. Testing
            // would be required to measure real performance characteristics.
            let count = raw_arc_handle_count(raw_handle)
                .checked_add(1)
                .unwrap_or_else(|| std::unreachable_unchecked());

            arc.init_count(count);

            std::drop(arc);
        }
    }
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Handle<T> {
    inner: u32,
    phantom: std::PhantomData<*const T>,
}

const SIZE_OF_U32: usize = std::size_of::<u32>();
const BIT_FIELD_WIDTH_TOTAL: usize = SIZE_OF_U32 * BITS_PER_BYTE;
const LOG_2_512: usize = 9;
const BIT_FIELD_WIDTH_COUNT: usize = LOG_2_512;
const BIT_FIELD_WIDTH_INDEX: usize =
    BIT_FIELD_WIDTH_TOTAL - BIT_FIELD_WIDTH_COUNT;
const BIT_SHIFT_COUNT: usize = 0;
const BIT_SHIFT_INDEX: usize = BIT_FIELD_WIDTH_INDEX;
const MASK_COUNT: u32 = !(u32::MAX << BIT_FIELD_WIDTH_COUNT);
const MASK_INDEX: u32 = !MASK_COUNT;

impl<T> Handle<T> {
    pub const fn count(&self) -> u32 {
        (self.inner & MASK_COUNT) >> BIT_FIELD_WIDTH_COUNT
    }

    pub const fn index(&self) -> u32 {
        (self.inner & MASK_INDEX) >> BIT_FIELD_WIDTH_INDEX
    }
}

#[repr(transparent)]
pub struct AtomicHandle<T> {
    inner: std::AtomicU32,
    phantom: std::PhantomData<*const T>,
}

impl<T> AtomicHandle<T> {
    pub const fn new(handle: Handle<T>) -> Self {
        Self {
            inner: std::AtomicU32::new(handle.inner),
            phantom: std::PhantomData,
        }
    }
}
