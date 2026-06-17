use core::ptr::null_mut;
use core::{arch::asm, ffi::c_void};

use crate::fixed_vaddrs::{
    APPLICATION_CORE_TSS_FIXED_VADDR, APPLICATION_CORE_TSS_RSP0_FIXED_VADDR_BASE,
};
use crate::{
    error,
    hal::{
        memory::allocator::{Allocator, MemoryBlock, VirtualAllocator},
        print::Module,
    },
    multithreading::processors::HOST_CORE_ACTIVATION_ID,
    success,
    utils::{intrin::wrmsr, memory::memcpy, traits::Region},
};

#[repr(packed)]
struct GdtSegment {
    limit_0: u16,
    base_0: u16,
    base_1: u8,
    access_byte: u8,
    flags_limit: u8,
    base_2: u8, //8 Byte
}

#[repr(packed)]
struct GdtSystemSegment {
    limit_0: u16,
    base_0: u16,
    base_1: u8,
    access_byte: u8,
    flags_limit: u8,
    base_2: u8,
    base_3: u32,
    buffer: u32,
}

struct TSS {
    reserved0: u32,
    rsp0_low: u32,
    rsp0_high: u32,
    rsp1_low: u32,
    rsp1_high: u32,
    rsp2_low: u32,
    rsp2_high: u32,
    reserved1: u32,
    reserved2: u32,
    ist1_low: u32,
    ist1_high: u32,
    ist2_low: u32,
    ist2_high: u32,
    ist3_low: u32,
    ist3_high: u32,
    ist4_low: u32,
    ist4_high: u32,
    ist5_low: u32,
    ist5_high: u32,
    ist6_low: u32,
    ist6_high: u32,
    ist7_low: u32,
    ist7_high: u32,
    reserved3: u32,
    reserved4: u32,
    reserved5: u16,
    iopb: u16,
}

#[repr(packed, C)]
pub struct GdtDescriptor {
    size: u16,
    offset: u64,
}

impl GdtDescriptor {
    pub fn standard(offset: u64) -> GdtDescriptor {
        let mut segments: *mut GdtSegment = offset as *mut GdtSegment;

        // Null
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).access_byte = 0;
            (*segments).flags_limit = 0;
            (*segments).limit_0 = 0;
            segments = segments.add(1);
        }
        // Kernel Code
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0x9A;
            (*segments).flags_limit = 0b10101111;
            segments = segments.add(1);
        }
        // Kernel Data
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0b10010010;
            (*segments).flags_limit = 0b10101111;
            segments = segments.add(1);
        }
        // User Data
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0b11110010;
            (*segments).flags_limit = 0b10101111;
            segments = segments.add(1);
        }
        // User Code
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0b11111010;
            (*segments).flags_limit = 0b10101111;
            segments = segments.add(1);
        }
        // Kernel Code 32 (40)
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0x9A;
            (*segments).flags_limit = 0b11101111;
            segments = segments.add(1);
        }
        // Kernel Data 32
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0b10010010;
            (*segments).flags_limit = 0b11101111;
        }
        return Self {
            size: (7 * 8) - 1,
            offset,
        };
    }

    pub fn initialize(
        &mut self,
        virtual_allocator: &mut Allocator,
        core_activation_id: u8,
        use_tss: bool,
    ) {
        let mut module = Module::new("GDT");
        let mut segments: *mut GdtSegment = match virtual_allocator.alloc_zero(1) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_) => {
                error!(module, "Could not allocate memory for gdt segments\n");
                unsafe { asm!("cli;hlt") };
                null_mut()
            }
        };
        let base = segments;
        // Null
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).access_byte = 0;
            (*segments).flags_limit = 0;
            (*segments).limit_0 = 0;
            segments = segments.add(1);
        }
        // Kernel Code
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0x9A;
            (*segments).flags_limit = 0b10101111;
            segments = segments.add(1);
        }
        // Kernel Data
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0b10010010;
            (*segments).flags_limit = 0b10101111;
            segments = segments.add(1);
        }

        // User Data
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0b1111_0010;
            (*segments).flags_limit = 0b10101111;
            segments = segments.add(1);
        }

        // User Code
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0b1111_1010;
            (*segments).flags_limit = 0b10101111;
            segments = segments.add(1);
        }

        // Kernel Code 32
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0x9A;
            (*segments).flags_limit = 0b11101111;
            segments = segments.add(1);
        }
        // Kernel Data 32
        unsafe {
            (*segments).base_0 = 0;
            (*segments).base_1 = 0;
            (*segments).base_2 = 0;
            (*segments).limit_0 = 0xFFFF;
            (*segments).access_byte = 0b10010010;
            (*segments).flags_limit = 0b11101111;
            segments = segments.add(1);
        }

        if use_tss {
            let system_descriptor = unsafe { &mut *(segments as *mut GdtSystemSegment) };
            system_descriptor.base_0 = (APPLICATION_CORE_TSS_FIXED_VADDR & 0xFFFF) as u16;
            system_descriptor.base_1 = ((APPLICATION_CORE_TSS_FIXED_VADDR >> 16) & 0xFF) as u8;
            system_descriptor.base_2 = ((APPLICATION_CORE_TSS_FIXED_VADDR >> 24) & 0xFF) as u8;
            system_descriptor.base_3 = (APPLICATION_CORE_TSS_FIXED_VADDR >> 32) as u32;
            system_descriptor.limit_0 = 103; // size_of::<TSS>() = 104
            system_descriptor.flags_limit = 0b0010_0000;
            system_descriptor.access_byte = 0b1000_1001;
            let tss = unsafe { &mut *(APPLICATION_CORE_TSS_FIXED_VADDR as *mut TSS) };
            tss.iopb = 0;
            tss.rsp0_low = (APPLICATION_CORE_TSS_RSP0_FIXED_VADDR_BASE & 0xFFFF_FFFF) as u32;
            tss.rsp0_high = (APPLICATION_CORE_TSS_RSP0_FIXED_VADDR_BASE >> 32) as u32;
            self.size = ((7 * 8) + 16) - 1;
        } else {
            self.size = (7 * 8) - 1;
        }
        self.offset = base as u64;
        unsafe {
            load_gdt(self);
            if use_tss {
                load_ltr();
            }
            // 0x10 + 16 => 0x20 (user cs) ; 0x10 + 8 => 0x18 (user ss)
            wrmsr(0xC0000081, (KERNEL_CODE_SEGMENT as u64) << 32 | 0x10 << 48);
        }
        success!(module, "Initialized on Core {}\n", core_activation_id);
    }
}

impl Default for GdtDescriptor {
    fn default() -> Self {
        return GdtDescriptor { size: 0, offset: 0 };
    }
}
#[unsafe(link_section = ".host_core")]
pub static mut HOST_CORE_GDT_DESCRIPTOR: GdtDescriptor = GdtDescriptor { size: 0, offset: 0 };

unsafe extern "C" {
    fn load_gdt(descriptor: *const GdtDescriptor);
    fn load_ltr();
    pub fn get_gdt_base() -> u64;
}

#[allow(unused)]
pub const KERNEL_CODE_SEGMENT: u16 = 0x8;
#[allow(unused)]
pub const KERNEL_DATA_SEGMENT: u16 = 0x10;
#[allow(unused)]
pub const USER_DATA_SEGMENT: u16 = 0x18;
#[allow(unused)]
pub const USER_CODE_SEGMENT: u16 = 0x20;
#[allow(unused)]
pub const KERNEL_CODE32_SEGMENT: u16 = 0x28;
#[allow(unused)]
pub const KERNEL_DATA32_SEGMENT: u16 = 0x30;
#[allow(unused)]
pub const TSS_SEGMENT: u16 = 0x38;

/* Info: only physical_allocator, since paging is only used later*/
pub fn initialize_host_core_gdt(physical_allocator: &mut Allocator) {
    unsafe {
        let gdt = &raw mut HOST_CORE_GDT_DESCRIPTOR;
        (*gdt).initialize(physical_allocator, HOST_CORE_ACTIVATION_ID, false);
    }
}

pub fn gdt_switch_address(
    physical_allocator: &mut Allocator,
    virt_allocator: &mut VirtualAllocator,
) {
    let mb = virt_allocator.allocator.alloc_zero(1).unwrap();
    unsafe {
        let prev_offset = HOST_CORE_GDT_DESCRIPTOR.offset;
        memcpy(
            mb.as_mut_ptr(),
            prev_offset as *const c_void,
            (HOST_CORE_GDT_DESCRIPTOR.size + 1) as u32,
        );
        physical_allocator
            .free(&MemoryBlock::new(0x1000, prev_offset))
            .unwrap();
        HOST_CORE_GDT_DESCRIPTOR.offset = mb.base;
        load_gdt(&raw const HOST_CORE_GDT_DESCRIPTOR);
    }
}
