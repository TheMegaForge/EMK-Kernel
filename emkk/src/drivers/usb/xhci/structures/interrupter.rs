use core::{ffi::c_void, ptr::null_mut};

use crate::{
    drivers::{
        disk::sata_abar::DeviceSleep,
        usb::xhci::{
            data_structures::XhciTrbId,
            registers::{
                XhciInterruptRegisterSet, XhciInterruptRegisterSetBitPart,
                XhciInterruptRegisterSetPart,
            },
            structures::{RawXhciTrb, XhciLinkTrb},
        },
    },
    hal::{
        memory::allocator::Allocator,
        print::{Module, simple_kernel_panic},
    },
    time::sleep,
    utils::traits::AsU64,
};
#[repr(align(16))]
pub struct XhciEventRingSegmentTableEntry {
    address: u64,
    size: u16,
}

/* er = event ring*/
pub struct XhciInterrupter {
    ir: XhciInterruptRegisterSet,
    er_segment_table: *mut XhciEventRingSegmentTableEntry,
    dequeue_pointer: *mut RawXhciTrb,
    end: *mut RawXhciTrb,
    css: bool,
    got_events: bool,
}

impl XhciInterrupter {
    pub const fn empty() -> Self {
        return Self {
            ir: XhciInterruptRegisterSet::new(null_mut()),
            er_segment_table: null_mut(),
            dequeue_pointer: null_mut(),
            end: null_mut(),
            css: true,
            got_events: false,
        };
    }
    #[inline(always)]
    pub fn clear_ip(&mut self) {
        self.ir.set(XhciInterruptRegisterSetBitPart::Ip, true);
    }
    #[inline(always)]
    pub fn clear_ehb(&mut self) {
        self.ir.set(XhciInterruptRegisterSetBitPart::Ehb, true);
    }
    pub fn new(mut ir: XhciInterruptRegisterSet, allocator: &mut Allocator) -> Self {
        ir.set(XhciInterruptRegisterSetBitPart::Ie, true);
        ir.set_part(XhciInterruptRegisterSetPart::ImodC, 4000);
        ir.set_part(XhciInterruptRegisterSetPart::EventRingSegmentTableSize, 1);
        let er_segment_table_mb = allocator.alloc_zero(1).unwrap();
        let er_segment_mb = allocator.alloc_zero(2).unwrap();
        unsafe {
            *er_segment_table_mb.as_mut_ptr::<u64>() = er_segment_mb.base;
            *(er_segment_table_mb.as_mut_ptr::<u64>().add(1) as *mut u16) = 512;
        };
        ir.set_event_ring_dequeue_pointer(er_segment_mb.base);
        ir.set_event_ring_table_base_address(er_segment_table_mb.base);
        ir.set(XhciInterruptRegisterSetBitPart::Ehb, false);
        return Self {
            ir,
            er_segment_table: er_segment_table_mb.as_mut_ptr(),
            dequeue_pointer: er_segment_mb.as_mut_ptr(),
            end: unsafe { er_segment_mb.as_mut_ptr::<RawXhciTrb>().add(512) },
            css: true,
            got_events: false,
        };
    }
    /**
     * second argument of the function is the trb type
     */
    pub fn consume_events(&mut self, mut consume_fn: impl FnMut(*mut RawXhciTrb, XhciTrbId)) {
        loop {
            if unsafe { self.dequeue_pointer.read_volatile()._dword3 } & 1 == self.css as u32 {
                let trb_type = XhciTrbId::from_u32(
                    (unsafe { self.dequeue_pointer.read_volatile()._dword3 } >> 10) & 0x3F,
                );
                (consume_fn)(self.dequeue_pointer, trb_type);
                self.got_events = true;
                self.dequeue_pointer = unsafe { self.dequeue_pointer.add(1) };
                if self.dequeue_pointer == self.end {
                    self.dequeue_pointer =
                        unsafe { &*self.er_segment_table }.address as *mut RawXhciTrb;
                    self.css = !self.css;
                }
            } else {
                self.ir
                    .set_event_ring_dequeue_pointer(self.dequeue_pointer as u64);
                return;
            }
        }
    }

    pub fn wait_for_events(&mut self) -> bool {
        let mut wait_counter = 20;
        while wait_counter > 0 {
            if self.got_events {
                self.got_events = false;
                return true;
            }
            wait_counter -= 1;
            sleep(10);
        }
        return false;
    }
    pub fn wait_for_events_or_crash(&mut self, module: &'static str, error: &'static str) {
        if !self.wait_for_events() {
            simple_kernel_panic(module, error);
        }
    }
}
