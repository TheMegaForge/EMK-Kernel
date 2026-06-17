use core::ptr::null_mut;

use crate::hal::memory::allocator::MemoryBlock;

pub struct List<T> {
    pub memory: MemoryBlock,
    pub ptr: *mut T,
    pub entries: u32,
}

impl<T> List<T> {
    pub const fn empty() -> Self {
        return Self {
            memory: MemoryBlock::empty(),
            ptr: null_mut(),
            entries: 0,
        };
    }
}

impl<T> Default for List<T> {
    fn default() -> Self {
        return Self {
            memory: MemoryBlock::default(),
            ptr: null_mut(),
            entries: 0,
        };
    }
}

impl<T> List<T> {
    pub fn size(&self) -> u32 {
        return self.entries;
    }

    pub fn ref_const(&self, index: u32) -> Option<&T> {
        if index > self.entries {
            return Option::None;
        }
        return unsafe { self.ptr.add(index as usize).as_ref() };
    }

    pub fn ref_mut(&mut self, index: u32) -> Option<&mut T> {
        if index > self.entries {
            return Option::None;
        }
        return unsafe { self.ptr.add(index as usize).as_mut() };
    }

    //operand returns false, if 'for_each' should break
    pub fn for_each(&self, mut operand: impl FnMut(u32, *const T) -> bool) {
        for i in 0..self.entries {
            if !(operand)(i, unsafe { self.ptr.add(i as usize) }) {
                break;
            }
        }
    }

    pub fn for_each_mut(&mut self, mut operand: impl FnMut(u32, *mut T) -> bool) {
        for i in 0..self.entries {
            if !(operand)(i, unsafe { self.ptr.add(i as usize) }) {
                break;
            }
        }
    }

    pub fn new(memory: MemoryBlock, entries: u32) -> Self {
        let ptr = memory.as_mut_ptr();
        return Self {
            memory,
            ptr,
            entries,
        };
    }
}
