use core::{
    ffi::{CStr, c_uchar, c_void},
    ptr::null_mut,
};

use crate::{
    hal::{
        memory::allocator::{Allocator, MemoryBlock},
        print::simple_kernel_panic,
    },
    utils::memory::memcpy,
};

pub struct String {
    buffer: MemoryBlock,
    current: *mut c_uchar,
    capacity: u16,
    length: u16,
}

impl Default for String {
    fn default() -> Self {
        return Self {
            buffer: MemoryBlock::default(),
            current: null_mut(),
            capacity: 0,
            length: 0,
        };
    }
}

impl String {
    pub fn new(allocator: &mut Allocator, mut capacity: u16) -> Self {
        let mut pages_needed = capacity / 0x1000;
        if capacity % 0x1000 != 0 {
            pages_needed += 1;
        }
        capacity = pages_needed * 0x1000;

        let base = match allocator.alloc_zero(pages_needed as u16) {
            Ok(mb) => mb,
            Err(_) => simple_kernel_panic("Stack/new", "Allocating failed\n"),
        };
        let current = base.as_mut_ptr();
        return String {
            buffer: base,
            current,
            capacity,
            length: 0,
        };
    }

    pub fn push_back(&mut self, to_append: &CStr) {
        if to_append.count_bytes() + self.length as usize > self.capacity as usize {
            simple_kernel_panic("String/push_back", "Overflow\n");
        }
        unsafe {
            memcpy(
                self.current as *mut c_void,
                to_append.as_ptr() as *const c_void,
                to_append.count_bytes() as u32,
            );
            self.current = self.current.add(to_append.count_bytes());
        }
        self.length += to_append.count_bytes() as u16;
    }
    //True => means same
    pub fn compare(&self, offset0: u16, offset1: u16, length: u16) -> bool {
        if offset0 + length > self.length || offset1 + length > self.length {
            return false;
        }

        let ptr0: *const c_uchar = unsafe { self.buffer.as_ptr::<c_uchar>().add(offset0 as usize) };
        let ptr1: *const c_uchar = unsafe { self.buffer.as_ptr::<c_uchar>().add(offset1 as usize) };

        for i in 0..length {
            if unsafe { *ptr0.add(i as usize) } != unsafe { *ptr1.add(i as usize) } {
                return false;
            }
        }
        return true;
    }
    pub fn compare_extern(&self, offset0: u16, ptr: *const c_uchar, length: u16) -> bool {
        if offset0 + length > self.length {
            return false;
        }

        let ptr0: *const c_uchar = unsafe { self.buffer.as_ptr::<c_uchar>().add(offset0 as usize) };

        for i in 0..length {
            if unsafe { *ptr0.add(i as usize) } != unsafe { *ptr.add(i as usize) } {
                return false;
            }
        }
        return true;
    }

    pub fn copy(&mut self, dest: u16, srce: u16, length: u16) {
        if dest + length > self.capacity || srce + length > self.capacity {
            simple_kernel_panic("String/copy", "Capacity overflow\n");
        }
        /* Increments the length, when copying into uninitialized memory*/
        if dest + length > self.length {
            self.length += (dest + length) - self.length;
        }

        unsafe {
            memcpy(
                self.buffer.as_mut_ptr::<c_void>().add(dest as usize),
                self.buffer.as_ptr::<c_void>().add(srce as usize),
                length as u32,
            );

            self.current = self.buffer.as_mut_ptr::<u8>().add(self.length as usize);
        }
    }

    pub fn get_current(&self) -> *const c_uchar {
        return self.current;
    }

    pub fn push_back_raw(&mut self, ptr: *const c_uchar, length: u16) {
        if self.length + length > self.capacity {
            simple_kernel_panic("String/push_back_raw", "Capacity overflow\n");
        }

        unsafe {
            memcpy(
                self.current as *mut c_void,
                ptr as *const c_void,
                length as u32,
            );

            self.current = self.current.add(length as usize);
        }
        self.length += length;
    }

    pub fn get_length(&self) -> u16 {
        return self.length;
    }
}
