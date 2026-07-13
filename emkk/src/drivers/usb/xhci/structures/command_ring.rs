use core::{
    ffi::{c_char, c_void},
    ptr::null_mut,
};

use crate::drivers::usb::xhci::{
    data_structures::XhciTrbId,
    registers::{XhciCrCr, XhciDoorbell},
    structures::XhciCommand,
};

pub struct XhciCommandRing {
    dequeue_pointer: *mut c_void,
    enqueue_pointer: *mut u32,
    crcr: XhciCrCr,
    doorbell: XhciDoorbell,
    /** producer cycle state*/
    pcs: bool,
}

impl XhciCommandRing {
    pub const fn empty() -> Self {
        return Self {
            dequeue_pointer: null_mut(),
            enqueue_pointer: null_mut(),
            crcr: XhciCrCr::new(null_mut()),
            doorbell: XhciDoorbell::new(null_mut()),
            pcs: true,
        };
    }
    pub fn new(doorbell: XhciDoorbell, crcr: XhciCrCr, initial_address: u64) -> Self {
        return Self {
            dequeue_pointer: initial_address as *mut c_void,
            enqueue_pointer: initial_address as *mut u32,
            crcr,
            doorbell,
            pcs: true,
        };
    }

    pub fn process(&mut self, command: XhciCommand) {
        match command {
            XhciCommand::NoOp => unsafe {
                self.enqueue_pointer
                    .add(3)
                    .write_volatile(self.pcs as u32 | (XhciTrbId::NoOpCommandTrb as u32) << 10)
            },
            XhciCommand::EnableSlot { slot_type } => unsafe {
                self.enqueue_pointer.add(3).write_volatile(
                    self.pcs as u32
                        | (XhciTrbId::EnableSlotCommand as u32) << 10
                        | (slot_type as u32) << 16,
                )
            },
            XhciCommand::AddressDevice {
                slot_id,
                input_context_address,
                bsr,
            } => unsafe {
                self.enqueue_pointer
                    .write_volatile((input_context_address & 0xFFFF_FFFF) as u32);
                self.enqueue_pointer
                    .add(1)
                    .write_volatile((input_context_address >> 32) as u32);
                self.enqueue_pointer.add(3).write_volatile(
                    (slot_id as u32) << 24
                        | (XhciTrbId::AddressDeviceCommand as u32) << 10
                        | (bsr as u32) << 9
                        | self.pcs as u32,
                );
            },
            XhciCommand::EvaluateContext {
                slot_id,
                input_context_address,
            } => unsafe {
                self.enqueue_pointer
                    .write_volatile((input_context_address & 0xFFFF_FFFF) as u32);
                self.enqueue_pointer
                    .add(1)
                    .write_volatile((input_context_address >> 32) as u32);
                self.enqueue_pointer.add(3).write_volatile(
                    (slot_id as u32) << 24
                        | (XhciTrbId::EvaluateContextCommand as u32) << 10
                        | self.pcs as u32,
                );
            },
            XhciCommand::ConfigureEndpoint {
                slot_id,
                input_context_address,
                dc,
            } => unsafe {
                self.enqueue_pointer
                    .write_volatile((input_context_address & 0xFFFF_FFFF) as u32);
                self.enqueue_pointer
                    .add(1)
                    .write_volatile((input_context_address >> 32) as u32);
                self.enqueue_pointer.add(3).write_volatile(
                    (slot_id as u32) << 24
                        | (XhciTrbId::ConfigureEndpointCommand as u32) << 10
                        | (dc as u32) << 9
                        | self.pcs as u32,
                );
            },
            XhciCommand::StopEndpoint {
                slot_id,
                endpoint_id,
                sp,
            } => unsafe {
                self.enqueue_pointer.add(3).write_volatile(
                    (slot_id as u32) << 24
                        | (sp as u32) << 23
                        | (endpoint_id as u32) << 16
                        | (XhciTrbId::StopEndpointCommand as u32) << 10
                        | self.pcs as u32,
                );
            },
            _ => todo!(),
        }
        self.enqueue_pointer = unsafe { self.enqueue_pointer.add(4) };
        self.doorbell.ring(0, 0);
    }
}
