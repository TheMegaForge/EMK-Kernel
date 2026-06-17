use core::ffi::c_void;

pub struct OhciNonPeriodicList {
    current: *mut u32,
    head: *mut u32,
}

impl OhciNonPeriodicList {
    pub const fn new(current: *mut u32, head: *mut u32) -> Self {
        return Self { current, head };
    }
}
