use crate::std;

pub struct Cell<T> {
    shared_ptr: std::AtomicPtr<T>,
    phantom: std::PhantomData<std::Mutex<T>>,
}

impl<T> Cell<T> {
    pub fn new(value: T) -> Self {
        Self {
            shared_ptr: std::AtomicPtr::new(std::Box::into_raw(std::Box::new(
                value,
            ))),
            phantom: std::PhantomData,
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
            let updated = self
                .origin
                .shared_ptr
                .swap(std::null_mut(), std::SeqCst)
                .as_mut()
                .map_or(false, |ptr| {
                    self.last_value = std::Some(*std::Box::from_raw(ptr));

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
            self.origin
                .shared_ptr
                .swap(std::Box::into_raw(std::Box::new(value)), std::SeqCst)
                .as_mut()
                .map(|ptr| std::drop(std::Box::from_raw(ptr)))
                .unwrap_or(())
        }
    }
}
