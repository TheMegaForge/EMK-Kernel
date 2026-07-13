use core::ffi::c_void;

use crate::hal::{
    memory::allocator::{Allocator, MemoryBlock},
    print::{Module, simple_kernel_panic},
};

unsafe extern "C" {
    pub fn memset(ptr: *mut c_void, value: u8, length: u32);
    pub fn memset_word(ptr: *mut c_void, value: u16, length: u32);
    pub fn memset_dword(ptr: *mut c_void, value: u32, length: u32);
    pub fn memset_qword(ptr: *mut c_void, value: u64, length: u32);

    pub fn memcpy(dst: *mut c_void, src: *const c_void, length: u32);
    pub fn memcpy_qword(dst: *mut c_void, src: *const c_void, length: u32);
}

#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn memcmp_dword_unaligned(ptr0: *const u32, ptr1: *const u32, length: u32) -> bool {
    for i in 0..length {
        if ptr0.add(i as usize).read_unaligned() != ptr1.add(i as usize).read_unaligned() {
            return false;
        }
    }
    return true;
}
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn memcmp_byte(ptr0: *const u8, ptr1: *const u8, length: u32) -> bool {
    for i in 0..length {
        if *ptr0.add(i as usize) != *ptr1.add(i as usize) {
            return false;
        }
    }
    true
}
#[inline(always)]
pub fn free_or_crash(
    allocator: &mut Allocator,
    mb: &MemoryBlock,
    module: &mut Module<'static>,
    message: &'static str,
) {
    if let Result::Err(_) = allocator.free(mb) {
        simple_kernel_panic(module.name(), message);
    }
}
#[inline(always)]
pub fn alloc_zero_or_crash(
    allocator: &mut Allocator,
    pages: u16,
    module: &mut Module<'static>,
    message: &'static str,
) -> MemoryBlock {
    match allocator.alloc_zero(pages) {
        Ok(mb) => return mb,
        Err(_) => simple_kernel_panic(module.name(), message),
    }
}

#[derive(Debug)]
pub enum MemoryResult {
    InvalidLength,
    InvalidAddress,
    InvalidBlock,
    InvalidFlags,
    InvalidActivateFlags,
    AllocationError,
    PagingError,
    Nospace,
}
