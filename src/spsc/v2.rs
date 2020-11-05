use crate::std;

pub struct Cell<T> {
    shared_allocator: Allocator<T>,
    shared_address: std::AtomicU8,
    phantom: std::PhantomData<std::Mutex<T>>,
}

impl<T> Cell<T> {
    pub fn new(value: T) -> Self {
        let shared_allocator: Allocator<T> = std::Default::default();
        let shared_address = std::AtomicU8::new(unsafe {
            shared_allocator.allocate_with(value).into_u8()
        });

        Self {
            shared_allocator,
            shared_address,
            phantom: std::Default::default(),
        }
    }

    pub fn split(&mut self) -> (Consumer<'_, T>, Producer<'_, T>) {
        let origin = &*self;

        (
            Consumer {
                origin,
                last_value: std::None,
            },
            Producer { origin },
        )
    }
}

pub struct Consumer<'a, T> {
    origin: &'a Cell<T>,
    last_value: std::Option<T>,
}

impl<'a, T> Consumer<'a, T> {
    pub fn get(&mut self) -> (bool, &T) {
        unsafe {
            let updated = std::NonZeroU8::new(
                self.origin.shared_address.swap(0, std::SeqCst),
            )
            .map_or(false, |address| {
                self.last_value = std::Some(
                    self.origin
                        .shared_allocator
                        .deallocate(Address::new(address.get())),
                );

                true
            });

            (
                updated,
                self.last_value
                    .as_ref()
                    .unwrap_or_else(|| std::unreachable_unchecked()),
            )
        }
    }
}

pub struct Producer<'a, T> {
    origin: &'a Cell<T>,
}

impl<'a, T> Producer<'a, T> {
    pub fn set(&mut self, value: T) {
        unsafe {
            std::NonZeroU8::new(self.origin.shared_address.swap(
                self.origin.shared_allocator.allocate_with(value).into_u8(),
                std::SeqCst,
            ))
            .map(|raw_address| {
                self.origin
                    .shared_allocator
                    .deallocate(Address::new(raw_address.get()))
            });
        }
    }
}

const FREE_ALL: u8 = 0b1110_0000;

struct Allocator<T> {
    free: crate::Align128<std::AtomicU8>,
    memory: [std::UnsafeCell<std::MaybeUninit<crate::Align128<T>>>; 3],
}

impl<T> std::Default for Allocator<T> {
    fn default() -> Self {
        Self {
            free: crate::Align128(std::AtomicU8::new(FREE_ALL)),
            memory: unsafe { std::MaybeUninit::uninit().assume_init() },
        }
    }
}

impl<T> Allocator<T> {
    unsafe fn allocate_with(&self, value: T) -> Address<T> {
        let mut address = std::MaybeUninit::<u8>::uninit();

        self.free.fetch_update(std::SeqCst, std::SeqCst, |free| {
            address.as_mut_ptr().write(free.leading_zeros() as u8);

            std::Some(free | (0b1000_0000 >> address.assume_init()))
        });

        let address = address.assume_init();

        (&mut *self.memory.get_unchecked(address as usize).get())
            .as_mut_ptr()
            .write(crate::Align128(value));

        Address::new(address)
    }

    unsafe fn deallocate(&self, address: Address<T>) -> T {
        use std::panic;

        std::todo!()
    }

    // unsafe fn deref(&self, address: &Address<T>) -> &T {
    //     &*(&*(&*self.memory.get_unchecked(address.value as usize).get())
    //         .as_ptr())
    // }
}

struct Address<T> {
    value: u8,
    phantom: std::PhantomData<*mut T>,
}

impl<T> Address<T> {
    fn new(value: u8) -> Self {
        Self {
            value,
            phantom: std::PhantomData,
        }
    }

    fn into_u8(self) -> u8 {
        self.value
    }
}
