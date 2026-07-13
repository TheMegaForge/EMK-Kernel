use core::ffi::c_void;

use crate::{
    drivers::usb::{
        ohci::structures::endpoint::{EndpointDescriptorBitPart::K, OhciEndpointDescriptor},
        standard_requests::UsbEndpointDescriptor,
    },
    hal::{
        memory::allocator::{Allocator, MemoryBlock},
        print::simple_kernel_panic,
    },
};

pub struct OhciNonPeriodicList {
    current: *mut u32,
    head: *mut u32,
    fill_ptr: *mut u32,
    fill_bit: u8,
    current_endpoint_index: u8,
    endpoint_memory: MemoryBlock,
}

impl OhciNonPeriodicList {
    pub const fn new(current: *mut u32, head: *mut u32, fill_ptr: *mut u32, fill_bit: u8) -> Self {
        return Self {
            current,
            head,
            fill_ptr,
            fill_bit,
            current_endpoint_index: 0,
            endpoint_memory: MemoryBlock::empty(),
        };
    }

    pub fn initialize(&mut self, physical_allocator: &mut Allocator) {
        self.endpoint_memory = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(
                "OhciNonPeriodicList/initialize",
                "Could not allocate memory for Endpoints\n",
            ),
        };
    }
    #[inline(always)]
    pub fn ep(&self, index: u8) -> OhciEndpointDescriptor {
        return OhciEndpointDescriptor::new(unsafe {
            self.endpoint_memory
                .as_mut_ptr::<c_void>()
                .add(16 * index as usize)
        });
    }

    pub fn append_endpoint(&mut self, endpoint: OhciEndpointDescriptor) -> u8 {
        let mut ep = self.ep(self.current_endpoint_index);
        ep.copy_from(&endpoint);
        if self.current_endpoint_index != 0 {
            let mut last_ep = self.ep(self.current_endpoint_index - 1);
            last_ep.write_next_ed(ep.address());
        }
        self.current_endpoint_index += 1;
        if self.current_endpoint_index as u16 + 1 > 255 {
            simple_kernel_panic(
                "OhciNonPeriodicList/append_endpoint",
                "Endpoint Count exhausted\n",
            )
        }
        return self.current_endpoint_index - 1;
    }
    /**
     * This will update *self.head, and set the specified fill bit
     */
    pub fn send_for_processing(&self) {
        unsafe { self.head.write_volatile(self.ep(0).address()) };
        let val = unsafe { self.fill_ptr.read_volatile() };
        unsafe {
            self.fill_ptr
                .write_volatile(val | (1 << self.fill_bit) as u32)
        }
    }
}
