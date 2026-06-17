use core::{ffi::c_void, ptr::null_mut};

use crate::{
    drivers::usb::ohci::structures::endpoint::{EndpointDescriptorBitPart, OhciEndpointDescriptor},
    hal::{
        memory::allocator::{Allocator, MemoryBlock},
        print::simple_kernel_panic,
    },
};

pub struct OhciHccaInterruptList {
    ptr: *mut u32,
    _32ms_ep_memory: MemoryBlock,
}

impl OhciHccaInterruptList {
    pub const fn new(ptr: *mut u32) -> Self {
        return Self {
            ptr,
            _32ms_ep_memory: MemoryBlock::empty(),
        };
    }
    #[inline(always)]
    fn ep(&self, index: u8) -> OhciEndpointDescriptor {
        return OhciEndpointDescriptor::new(unsafe { *self.ptr.add(index as usize) } as *mut c_void);
    }

    /**
     *  Creates the Tree
     *  All EP created are skiped tho.
     */
    pub fn initialize(&mut self, physical_allocator: &mut Allocator) {
        let mb = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(
                "OhciHccaInterruptList/initialize",
                "Could not allocate memory for 32ms endpoints",
            ),
        };

        let mut _1ms_connector = OhciEndpointDescriptor::new(mb.base as *mut c_void);
        let mut _2ms_connector = OhciEndpointDescriptor::new((mb.base + 1 * 16) as *mut c_void);
        let mut _4ms_connector = OhciEndpointDescriptor::new((mb.base + 2 * 16) as *mut c_void);
        let mut _8ms_connector = OhciEndpointDescriptor::new((mb.base + 3 * 16) as *mut c_void);
        let mut _16ms_connector = OhciEndpointDescriptor::new((mb.base + 4 * 16) as *mut c_void);

        _1ms_connector.set(EndpointDescriptorBitPart::K, true);
        _1ms_connector.set(EndpointDescriptorBitPart::Con, true);
        _2ms_connector.set(EndpointDescriptorBitPart::K, true);
        _2ms_connector.set(EndpointDescriptorBitPart::Con, true);
        _4ms_connector.set(EndpointDescriptorBitPart::K, true);
        _4ms_connector.set(EndpointDescriptorBitPart::Con, true);
        _8ms_connector.set(EndpointDescriptorBitPart::K, true);
        _8ms_connector.set(EndpointDescriptorBitPart::Con, true);
        _16ms_connector.set(EndpointDescriptorBitPart::K, true);
        _16ms_connector.set(EndpointDescriptorBitPart::Con, true);

        _16ms_connector.write_next_ed(_8ms_connector.address());
        _8ms_connector.write_next_ed(_4ms_connector.address());
        _4ms_connector.write_next_ed(_2ms_connector.address());
        _2ms_connector.write_next_ed(_1ms_connector.address());

        for i in 0..32 {
            let mut ep = OhciEndpointDescriptor::new((mb.base + (i + 5) * 16) as *mut c_void); //allready zero´ed out
            ep.set(EndpointDescriptorBitPart::K, true);
            unsafe { *self.ptr.add(i as usize) = ep.address() };
        }

        self.ep(0).write_next_ed(_16ms_connector.address());
        self.ep(2).write_next_ed(_2ms_connector.address());
        self.ep(4).write_next_ed(_4ms_connector.address());
        self.ep(6).write_next_ed(_2ms_connector.address());
        self.ep(8).write_next_ed(_8ms_connector.address());
        self.ep(10).write_next_ed(_2ms_connector.address());
        self.ep(12).write_next_ed(_4ms_connector.address());
        self.ep(14).write_next_ed(_2ms_connector.address());
        self.ep(16).write_next_ed(_16ms_connector.address());
        self.ep(18).write_next_ed(_2ms_connector.address());
        self.ep(20).write_next_ed(_4ms_connector.address());
        self.ep(22).write_next_ed(_2ms_connector.address());
        self.ep(24).write_next_ed(_8ms_connector.address());
        self.ep(26).write_next_ed(_2ms_connector.address());
        self.ep(28).write_next_ed(_4ms_connector.address());
        self.ep(30).write_next_ed(_2ms_connector.address());

        for i in 0..32 {
            if i % 2 == 1 {
                self.ep(i).write_next_ed(_1ms_connector.address());
            }
        }

        self._32ms_ep_memory = mb;
    }
}
