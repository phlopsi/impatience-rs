use crate::std;

#[derive(Copy, Clone)]
pub struct Hole {
    next: *mut Hole,
    size: usize,
}

#[repr(align(128))]
pub union Slot<T>
where
    T: std::Copy,
{
    uninit: (),
    hole: Hole,
    element: std::ManuallyDrop<T>,
}

impl<T> Slot<T>
where
    T: std::Copy,
{
    pub fn uninit() -> Self {
        Self { uninit: () }
    }

    pub fn hole(hole: Hole) -> Self {
        Self { hole }
    }

    pub fn element(element: T) -> Self {
        Self {
            element: std::ManuallyDrop::new(element),
        }
    }

    pub unsafe fn as_hole(&self) -> &Hole {
        &self.hole
    }

    pub unsafe fn as_element(&self) -> &T {
        &self.element
    }
}

pub struct Layout<T>
where
    T: std::Copy,
{
    inner: std::Layout,
    phantom: std::PhantomData<T>,
}

impl<T> Layout<T>
where
    T: std::Copy,
{
    pub fn array(n: usize) -> Self {
        const MIN_SIZE: usize = 128;
        const MAX_SIZE: usize = (usize::MAX >> 1) + 1;

        let size = std::size_of::<Slot<T>>() * n;

        {
            use std::panic;

            std::assert!(MIN_SIZE <= size, size <= MAX_SIZE);
        }

        let align = {
            let tmp = (size << 1) - 1;
            let mask = !(usize::MAX >> tmp.leading_zeros() as usize + 1);

            tmp & mask
        };

        Self {
            inner: unsafe {
                std::Layout::from_size_align_unchecked(size, align)
            },
            phantom: std::PhantomData,
        }
    }
}

pub struct Allocator<T>
where
    T: std::Copy,
{
    memory_block: *mut Slot<T>,
}

impl<T> Allocator<T>
where
    T: std::Copy,
{
    pub fn new(layout: Layout<T>) -> Self {
        Self {
            memory_block: unsafe { std::alloc(layout.inner) } as *mut _,
        }
    }
}
