use core::{ptr, slice};

#[inline(always)]
pub fn resize_slice<T>(slice: &mut &mut [T], new_size: usize) {
    *slice = unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr(), new_size) }
}

#[inline(always)]
pub fn rebase_slice<T>(slice: &mut &mut [T], new_base: u64) {
    *slice = unsafe { slice::from_raw_parts_mut(new_base as *mut T, slice.len()) };
}

#[inline(always)]
pub const fn invalid_mut_slice<'a, T>() -> &'a mut [T] {
    return unsafe { slice::from_raw_parts_mut(align_of::<T>() as *mut T, 0) };
}

#[inline(always)]
pub const fn invalid_slice<'a, T>() -> &'a [T] {
    return unsafe { slice::from_raw_parts(align_of::<T>() as *const T, 0) };
}

#[inline(always)]
pub fn slice_end_address<T>(slice: &[T]) -> u64 {
    return ptr::from_ref(&slice[slice.len() - 1]).addr() as u64;
}

#[inline(always)]
pub fn change_mut_slice_size<'a, T>(slice: &'a mut [T], new_size: usize) -> &'a mut [T] {
    unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr(), new_size) }
}

#[inline(always)]
pub fn change_slice_size<'a, T>(slice: &'a [T], new_size: usize) -> &'a [T] {
    unsafe { slice::from_raw_parts(slice.as_ptr(), new_size) }
}
