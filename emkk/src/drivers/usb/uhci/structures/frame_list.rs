use core::{
    mem::transmute,
    ptr::{addr_eq, null_mut},
};

use crate::{
    drivers::usb::{
        ehci::data_structures::QueueHead,
        independent::UsbTransferType,
        uhci::data_structures::{
            RawUhciQueueHead, RawUhciTransferDescriptor, UhciQueueHead, UhciTransferDescriptor,
            UhciTransferDescriptorBitPart,
        },
    },
    hal::{
        memory::allocator::Allocator,
        print::{Module, simple_kernel_panic},
    },
    utils::{
        memory::alloc_zero_or_crash,
        slices::{invalid_mut_slice, invalid_slice, slice_end_address},
    },
};
#[repr(align(16))]
pub struct UhciFrameListRoot {
    employ: *mut u32,
    index: u32,
}

impl UhciFrameListRoot {
    /** Transfer Descriptors have priority so they always come first
     */
    pub fn append_td(&mut self, mut td: UhciTransferDescriptor) {
        if (unsafe { self.employ.read_volatile() }) & 1 == 1 {
            let next_value = unsafe { self.employ.read_volatile() };
            td.set(UhciTransferDescriptorBitPart::T, 1 == next_value & 1);
            td.set(UhciTransferDescriptorBitPart::Q, 2 == next_value & 2);
            td.set(UhciTransferDescriptorBitPart::Vf, false);
            td.write_link_pointer(next_value & !0xF);
            unsafe { self.employ.write_volatile(td.address()) };
            return;
        }
        let mut current = unsafe { self.employ.read_volatile() };
        let mut previous = unsafe { self.employ.read_volatile() };
        let append_to_ptr;
        loop {
            if current == td.address() {
                return;
            }
            if current & 1 == 1 {
                append_to_ptr = (current & !0xF) as *mut u32;
                break;
            }
            previous = current;
            current = unsafe { (current as *mut u32).read_volatile() }
        }
        let next_value = unsafe { append_to_ptr.read_volatile() };
        td.set(UhciTransferDescriptorBitPart::T, 1 == next_value & 1);
        td.set(UhciTransferDescriptorBitPart::Q, 2 == next_value & 2);
        td.set(UhciTransferDescriptorBitPart::Vf, false);
        td.write_link_pointer(next_value & !0xF);
        unsafe { append_to_ptr.write_volatile(td.address()) };
    }
    /**
     * Qh are appended in an order: Interrupt/Control then Bulk
     */
    pub fn append_qh(
        &mut self,
        qh: &mut UhciQueueHead,
        transfer_type: UsbTransferType,
        interrupt_qhs: &[RawUhciQueueHead],
        control_qhs: &[RawUhciQueueHead],
    ) {
        let mut append_to_ptr = null_mut();
        match transfer_type {
            UsbTransferType::Isochronous => {
                simple_kernel_panic(
                    #[allow(static_mut_refs)]
                    unsafe {
                        UHCI_FRAME_LIST_MODULE.name()
                    },
                    "Tried to insert a QueueHead with transfer_type = Isochronous\n",
                );
            }
            UsbTransferType::Interrupt => {
                if (unsafe { self.employ.read_volatile() }) & 1 == 1 {
                    append_to_ptr = self.employ;
                } else {
                    let mut current = unsafe { self.employ.read_volatile() };
                    let mut previous = unsafe { self.employ.read_volatile() };
                    loop {
                        if current & !0xF == qh.address() {
                            return;
                        }
                        if current & 1 == 1 {
                            append_to_ptr = (previous & !0xF) as *mut u32;
                            break;
                        }
                        previous = current;
                        current = unsafe { ((current & !0xF) as *mut u32).read_volatile() };
                    }
                }
            }
            UsbTransferType::Control => {
                if (unsafe { self.employ.read_volatile() }) & 1 == 1 {
                    append_to_ptr = self.employ;
                } else {
                    let mut current = unsafe { self.employ.read_volatile() };
                    let mut previous = unsafe { self.employ.read_volatile() };
                    loop {
                        if current & !0xF == qh.address() {
                            return;
                        }
                        if current & 1 == 1
                            || ((current & !0xF >= interrupt_qhs.as_ptr().addr() as u32)
                                && slice_end_address(interrupt_qhs) as u32 >= (current & !0xF))
                        {
                            append_to_ptr = (previous & !0xF) as *mut u32;
                            break;
                        }
                        previous = current;
                        current = unsafe { ((current & !0xF) as *mut u32).read_volatile() };
                    }
                }
            }
            UsbTransferType::Bulk => {
                if (unsafe { self.employ.read_volatile() }) & 1 == 1 {
                    append_to_ptr = self.employ;
                } else {
                    let mut current = unsafe { self.employ.read_volatile() };
                    let mut previous = unsafe { self.employ.read_volatile() };
                    loop {
                        if current & !0xF == qh.address() {
                            return;
                        }
                        if current & 1 == 1
                            || ((current & !0xF >= control_qhs.as_ptr().addr() as u32)
                                && slice_end_address(control_qhs) as u32 >= (current & !0xF))
                        {
                            append_to_ptr = (previous & !0xF) as *mut u32;
                            break;
                        }
                        previous = current;
                        current = unsafe { ((current & !0xF) as *mut u32).read_volatile() };
                    }
                }
            }
        }
        let next_val = unsafe { append_to_ptr.read_volatile() };
        qh.set_queue_head_link_pointer(next_val & !0xF);
        qh.set_queue_head_link_q(next_val & 2 == 2);
        qh.set_queue_head_link_t(next_val & 1 == 1);
        unsafe { append_to_ptr.write_volatile(qh.address() | 2) };
    }
}

pub struct UhciFrameList {
    roots: &'static mut [UhciFrameListRoot],
    interrupt_qhs: &'static [RawUhciQueueHead],
    control_qhs: &'static [RawUhciQueueHead],
}

pub static mut UHCI_FRAME_LIST_MODULE: Module<'static> = Module::new("UhciFrameList");
impl UhciFrameList {
    pub const CONTROL_ENDPOINT_CALL_TIME: u32 = 16; // 16ms

    pub const fn empty() -> Self {
        return Self {
            roots: invalid_mut_slice(),
            interrupt_qhs: invalid_slice(),
            control_qhs: invalid_slice(),
        };
    }

    pub fn new(
        employ_roots: *mut u32,
        allocator: &mut Allocator,
        interrupt_qhs: &'static [RawUhciQueueHead],
        control_qhs: &'static [RawUhciQueueHead],
    ) -> Self {
        #[allow(static_mut_refs)]
        let r#mod = unsafe { &mut UHCI_FRAME_LIST_MODULE };
        let roots_mb = alloc_zero_or_crash(allocator, 4, r#mod, "Could not allocate root Array\n");
        let roots = roots_mb
            .as_mut_slice_limited::<UhciFrameListRoot>(0, 1024)
            .unwrap();
        for (i, root) in (&mut *roots).iter_mut().enumerate() {
            root.employ = unsafe { employ_roots.add(i) };
            root.index = i as u32;
        }
        return Self {
            interrupt_qhs,
            control_qhs,
            roots,
        };
    }

    pub fn addr(&self) -> u32 {
        return self.roots[0].employ.addr() as u32;
    }

    pub fn mark_invalid(&mut self) {
        for root in &mut *self.roots {
            unsafe { root.employ.write_volatile(1) }
        }
    }

    pub fn place_qh(
        &mut self,
        qh: &mut UhciQueueHead,
        interval: u32,
        transfer_type: UsbTransferType,
    ) {
        for i in 1..(1024 / interval) {
            self.roots[i as usize * interval as usize].append_qh(
                qh,
                transfer_type,
                self.interrupt_qhs,
                self.control_qhs,
            );
        }
    }
}
