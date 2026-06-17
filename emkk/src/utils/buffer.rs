use core::{ffi::c_void, ptr::null_mut, slice};

use crate::{
    acpi_tables::AcpiTableId::SRAT,
    hal::{
        memory::allocator::{Allocator, MemoryBlock, VirtualAllocator},
        print::simple_kernel_panic,
    },
    utils::traits::Region,
};

pub struct Buffer {
    ptr: *mut u8,
    allocated_size: u64,
    original_size: u64,
    readonly: bool,
}

impl Buffer {
    pub fn empty() -> Self {
        return Self {
            ptr: null_mut(),
            allocated_size: 0,
            original_size: 0,
            readonly: true,
        };
    }

    pub fn new_physical_virtual(
        allocator: &mut VirtualAllocator,
        readonly: bool,
        mut allocated_size: u64,
    ) -> (Self, MemoryBlock) {
        let original_size = allocated_size;
        if allocated_size % 0x1000 != 0 {
            allocated_size += 0x1000 - (allocated_size % 0x1000);
        }

        let mb = match allocator.allocator.alloc((allocated_size / 0x1000) as u16) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic("Buffer/new_physical_virtual", "Could not allocate\n"),
        };

        let offset = mb.base - allocator.allocator.lowest_address();

        return (
            Self {
                ptr: (allocator.physical.base + offset) as *mut u8,
                readonly,
                original_size,
                allocated_size,
            },
            mb,
        );
    }

    pub fn new(allocator: &mut Allocator, readonly: bool, mut allocated_size: u64) -> Self {
        let original_size = allocated_size;
        if allocated_size % 0x1000 != 0 {
            allocated_size += 0x1000 - (allocated_size % 0x1000);
        }

        let ptr = match allocator.alloc((allocated_size / 0x1000) as u16) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => simple_kernel_panic("Buffer/new", "Could not allocate\n"),
        };
        return Self {
            ptr,
            allocated_size,
            original_size,
            readonly,
        };
    }

    pub fn sub_buffer(&self, offset: u64, length: u64) -> Option<Buffer> {
        if offset > self.original_size {
            return Option::None;
        }
        return Option::Some(Buffer {
            ptr: unsafe { self.ptr.add(offset as usize) },
            allocated_size: 0,
            original_size: length,
            readonly: self.readonly,
        });
    }

    pub fn from_existing(
        address: u64,
        allocated_size: u64,
        original_size: u64,
        readonly: bool,
    ) -> Buffer {
        return Self {
            ptr: address as *mut u8,
            allocated_size,
            original_size,
            readonly,
        };
    }

    pub fn address(&self) -> u64 {
        return self.ptr as u64;
    }

    pub fn get_size(&self) -> u64 {
        return self.original_size;
    }

    pub fn is_readonly(&self) -> bool {
        return self.readonly;
    }

    pub fn as_const(&self) -> *const u8 {
        return self.ptr;
    }

    pub fn as_mut(&self) -> Option<*mut u8> {
        if self.readonly {
            return Option::None;
        }
        return Option::Some(self.ptr);
    }

    pub fn as_slice(&self) -> &[u8] {
        return unsafe { slice::from_raw_parts(self.ptr, self.original_size as usize) };
    }

    pub fn as_mut_slice(&self) -> Option<&mut [u8]> {
        if self.readonly {
            return Option::None;
        }
        return unsafe {
            Option::Some(slice::from_raw_parts_mut(
                self.ptr,
                self.original_size as usize,
            ))
        };
    }

    pub fn release(&mut self, allocator: &mut Allocator) {
        match allocator.free(&MemoryBlock::new(
            self.allocated_size as u64,
            self.ptr as u64,
        )) {
            Ok(_) => {}
            Err(_e) => simple_kernel_panic("Buffer/release", "Could not release\n"),
        };
        self.ptr = null_mut();
        self.readonly = true;
        self.allocated_size = 0;
        self.original_size = 0;
    }
}
