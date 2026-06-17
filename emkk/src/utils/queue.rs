use core::ptr::null_mut;

use crate::hal::{
    memory::allocator::{Allocator, PointerBlock},
    print::simple_kernel_panic,
};

use crate::utils::traits::Region;

pub struct Queue<T: Copy> {
    base: PointerBlock<T>,
    capacity: u16,
    inserted: u16,
    current_enqueue: u16,
    current_dequeue: u16,
}
// FIFO Principle => First In First Out
impl<T: Copy> Queue<T> {
    pub const fn empty() -> Self {
        return Self {
            base: PointerBlock::<T> {
                base: null_mut(),
                length: 0,
            },
            capacity: 0,
            inserted: 0,
            current_enqueue: 0,
            current_dequeue: 0,
        };
    }
}

impl<T: Copy> Default for Queue<T> {
    fn default() -> Self {
        return Self {
            base: PointerBlock::<T>::default(),
            capacity: 0,
            inserted: 0,
            current_enqueue: 0,
            current_dequeue: 0,
        };
    }
}

impl<T: Copy> Queue<T> {
    pub fn new(allocator: &mut Allocator, mut capacity: u16) -> Self {
        let size_of_entry = size_of::<T>() as u16;
        let bytes_needed = capacity * size_of_entry;

        let mut pages_needed = bytes_needed / 0x1000;
        if bytes_needed % 0x1000 != 0 {
            pages_needed += 1;
        }
        capacity = (pages_needed * 0x1000) / size_of_entry;

        let base = match allocator.alloc_zero(pages_needed as u16) {
            Ok(mb) => mb,
            Err(_) => simple_kernel_panic("Stack/new", "Allocating failed\n"),
        };
        return Queue::<T> {
            base: PointerBlock::new(base.get_length(), base.get_base() as *mut T),
            capacity,
            inserted: 0,
            current_dequeue: 0,
            current_enqueue: 0,
        };
    }

    pub fn num_occupied(&self) -> u16 {
        return self.inserted;
    }

    fn get_at_dequeue(&mut self) -> *mut T {
        return unsafe { self.base.as_mut_ptr().add(self.current_dequeue as usize) };
    }
    fn get_at_enqueue(&mut self) -> *mut T {
        return unsafe { self.base.as_mut_ptr().add(self.current_enqueue as usize) };
    }

    pub fn enqueue(&mut self, val: T) {
        self.inserted += 1;
        unsafe {
            *self.get_at_enqueue() = val;
        }
        self.current_enqueue = self.current_enqueue + 1 % self.capacity;
    }
    pub fn dequeue(&mut self) -> T {
        self.inserted -= 1;
        let ret = unsafe { *self.get_at_dequeue() };
        self.current_dequeue = self.current_dequeue + 1 % self.capacity;
        return ret;
    }
    pub fn dequeue_silent(&mut self) {
        self.inserted -= 1;
        self.current_dequeue = self.current_dequeue + 1 % self.capacity;
    }
}
