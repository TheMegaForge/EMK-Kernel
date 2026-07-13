use core::{f32::consts::LOG2_10, ffi::c_void, ptr::null_mut, slice};

use crate::{
    drivers::usb::ohci::structures::endpoint::{
        EndpointDescriptorBitPart, OhciEndpointDescriptor, RawOhciEndpointDescriptor,
    },
    hal::{
        memory::allocator::{Allocator, MemoryBlock},
        print::simple_kernel_panic,
    },
    utils::slices::invalid_mut_slice,
};

pub struct OhciHccaInterruptList {
    hcca_ptr: *mut u32,
    endpoints: &'static mut [RawOhciEndpointDescriptor],
    memory: MemoryBlock,
    endpoints_allocated: u8,
}

impl OhciHccaInterruptList {
    pub const fn new(hcca_ptr: *mut u32) -> Self {
        return Self {
            hcca_ptr,
            memory: MemoryBlock::empty(),
            endpoints: invalid_mut_slice(),
            endpoints_allocated: 0,
        };
    }
    #[inline(always)]
    fn hcca_ep(&self, index: u8) -> OhciEndpointDescriptor {
        return OhciEndpointDescriptor::new(
            unsafe { *self.hcca_ptr.add(index as usize) } as *mut c_void
        );
    }

    pub fn ep(&self, index: u8) -> OhciEndpointDescriptor {
        return OhciEndpointDescriptor::new(
            (&raw const self.endpoints[index as usize]) as *mut c_void,
        );
    }

    /**
     * epd.next_td should not be set
     * epd.Dum must not be set
     * Returns a value which can be used by ep()
     */
    pub fn install(&mut self, interval: u8, epd: OhciEndpointDescriptor) -> u16 {
        if interval == 32 {
            for i in 0..32 {
                let mut ep = self.hcca_ep(i);
                if ep.is_set(EndpointDescriptorBitPart::Dum) {
                    let next_ed = ep.next_ed();
                    ep.copy_from(&epd);
                    ep.write_next_ed(next_ed);
                    return i as u16 + 5;
                }
            }

            let comes_first = !epd.is_set(EndpointDescriptorBitPart::F);

            let mut append_ed = self.hcca_ep(0);

            while append_ed.next_ed() != 0
                && !OhciEndpointDescriptor::new(append_ed.next_ed() as *mut c_void)
                    .is_set(EndpointDescriptorBitPart::Con)
            {
                if comes_first
                    && OhciEndpointDescriptor::new(append_ed.next_ed() as *mut c_void)
                        .is_set(EndpointDescriptorBitPart::F)
                {
                    break;
                }
                append_ed = OhciEndpointDescriptor::new(append_ed.next_ed() as *mut c_void);
            }

            let mut dst_ep = OhciEndpointDescriptor::new(
                (&raw mut self.endpoints[self.endpoints_allocated as usize]) as *mut c_void,
            );
            dst_ep.copy_from(&epd);
            dst_ep.write_next_ed(append_ed.next_ed());
            append_ed.write_next_ed(dst_ep.address());

            self.endpoints_allocated += 1;
            return self.endpoints_allocated as u16 - 1;
        } else {
            assert!(interval > 0 && interval.is_power_of_two());
            let index = interval.ilog2();
            let mut append_ed = self.ep(index as u8);

            let comes_first = !epd.is_set(EndpointDescriptorBitPart::F);

            while append_ed.next_ed() != 0
                && !OhciEndpointDescriptor::new(append_ed.next_ed() as *mut c_void)
                    .is_set(EndpointDescriptorBitPart::Con)
            {
                if comes_first
                    && OhciEndpointDescriptor::new(append_ed.next_ed() as *mut c_void)
                        .is_set(EndpointDescriptorBitPart::F)
                {
                    break;
                }
                append_ed = OhciEndpointDescriptor::new(append_ed.next_ed() as *mut c_void);
            }

            let mut dst_ep = OhciEndpointDescriptor::new(
                (&raw mut self.endpoints[self.endpoints_allocated as usize]) as *mut c_void,
            );
            dst_ep.copy_from(&epd);
            dst_ep.write_next_ed(append_ed.next_ed());
            append_ed.write_next_ed(dst_ep.address());

            self.endpoints_allocated += 1;
            return self.endpoints_allocated as u16 - 1;
        }
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

        self.endpoints = unsafe {
            slice::from_raw_parts_mut(
                mb.as_mut_ptr(),
                0x1000 / size_of::<RawOhciEndpointDescriptor>(),
            )
        };

        let mut _1ms_connector =
            OhciEndpointDescriptor::new((&raw mut self.endpoints[0]) as *mut c_void);
        let mut _2ms_connector =
            OhciEndpointDescriptor::new((&raw mut self.endpoints[1]) as *mut c_void);
        let mut _4ms_connector =
            OhciEndpointDescriptor::new((&raw mut self.endpoints[2]) as *mut c_void);
        let mut _8ms_connector =
            OhciEndpointDescriptor::new((&raw mut self.endpoints[3]) as *mut c_void);
        let mut _16ms_connector =
            OhciEndpointDescriptor::new((&raw mut self.endpoints[4]) as *mut c_void);

        self.endpoints_allocated = 5;

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
            let mut ep = OhciEndpointDescriptor::new(
                (&raw mut self.endpoints[self.endpoints_allocated as usize]) as *mut c_void,
            );
            self.endpoints_allocated += 1;
            ep.set(EndpointDescriptorBitPart::K, true);
            ep.set(EndpointDescriptorBitPart::Dum, true);
            unsafe { *self.hcca_ptr.add(i as usize) = ep.address() };
        }
        self.hcca_ep(0).write_next_ed(_16ms_connector.address());
        self.hcca_ep(2).write_next_ed(_2ms_connector.address());
        self.hcca_ep(4).write_next_ed(_4ms_connector.address());
        self.hcca_ep(6).write_next_ed(_2ms_connector.address());
        self.hcca_ep(8).write_next_ed(_8ms_connector.address());
        self.hcca_ep(10).write_next_ed(_2ms_connector.address());
        self.hcca_ep(12).write_next_ed(_4ms_connector.address());
        self.hcca_ep(14).write_next_ed(_2ms_connector.address());
        self.hcca_ep(16).write_next_ed(_16ms_connector.address());
        self.hcca_ep(18).write_next_ed(_2ms_connector.address());
        self.hcca_ep(20).write_next_ed(_4ms_connector.address());
        self.hcca_ep(22).write_next_ed(_2ms_connector.address());
        self.hcca_ep(24).write_next_ed(_8ms_connector.address());
        self.hcca_ep(26).write_next_ed(_2ms_connector.address());
        self.hcca_ep(28).write_next_ed(_4ms_connector.address());
        self.hcca_ep(30).write_next_ed(_2ms_connector.address());

        for i in 0..32 {
            if i % 2 == 1 {
                self.hcca_ep(i).write_next_ed(_1ms_connector.address());
            }
        }
        self.memory = mb;
    }
}
