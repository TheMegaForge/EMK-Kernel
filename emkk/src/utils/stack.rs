use core::ptr::null_mut;

use crate::{
    hal::{
        memory::allocator::{Allocator, PointerBlock},
        print::simple_kernel_panic,
    },
    utils::traits::Region,
};

pub struct Stack<T: Copy> {
    base: PointerBlock<T>,
    capacity: u16,
    inserted: u16,
}
// FILO Principle => First In Last Out
impl<T: Copy> Stack<T> {
    pub const fn empty() -> Self {
        return Self {
            base: PointerBlock::<T> {
                base: null_mut(),
                length: 0,
            },
            capacity: 0,
            inserted: 0,
        };
    }
}

impl<T: Copy> Default for Stack<T> {
    fn default() -> Self {
        return Self {
            base: PointerBlock::<T>::default(),
            capacity: 0,
            inserted: 0,
        };
    }
}

impl<T: Copy> Stack<T> {
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
        return Stack::<T> {
            base: PointerBlock::new(base.get_length(), base.get_base() as *mut T),
            capacity,
            inserted: 0,
        };
    }

    pub fn num_occupied(&self) -> u16 {
        return self.inserted;
    }

    pub fn get_top(&self) -> *const T {
        return unsafe {
            self.base
                .as_ptr()
                .add((self.inserted - (1 * (self.inserted != 0) as u16)) as usize)
        };
    }

    pub fn for_each(&self, mut operand: impl FnMut(u32, *const T) -> bool) {
        for i in 0..self.inserted as u32 {
            if !(operand)(i, unsafe { self.base.as_ptr().add(i as usize) }) {
                break;
            }
        }
    }

    pub fn get_mut_top(&mut self) -> *mut T {
        return unsafe {
            self.base
                .as_mut_ptr()
                .add((self.inserted - (1 * (self.inserted != 0) as u16)) as usize)
        };
    }

    pub fn ref_top(&self) -> &T {
        return unsafe { self.get_top().as_ref().unwrap() };
    }

    pub fn mut_top(&mut self) -> &mut T {
        return unsafe { self.get_mut_top().as_mut().unwrap() };
    }
    pub fn push(&mut self, val: T) {
        if self.inserted + 1 > self.capacity {
            simple_kernel_panic("Stack/push", "Stack Overflow\n");
        }
        unsafe {
            *self.base.as_mut_ptr().add(self.inserted as usize) = val;
        }
        self.inserted += 1;
    }

    pub fn pop_silent(&mut self) {
        if self.inserted == 0 {
            simple_kernel_panic("Stack/pop", "Stack Underflow\n");
        }
        self.inserted -= 1;
    }

    pub fn pop(&mut self) -> T {
        if self.inserted == 0 {
            simple_kernel_panic("Stack/pop", "Stack Underflow\n");
        }
        let ret = unsafe { *self.get_mut_top() };
        self.inserted -= 1;
        return ret;
    }
}
