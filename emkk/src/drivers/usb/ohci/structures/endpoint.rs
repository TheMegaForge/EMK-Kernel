use core::{ffi::c_void, ptr::null_mut};

use crate::drivers::usb::traits::UsbEndpoint;

pub struct OhciEndpointDescriptor {
    val: *mut u32,
}

/*
 * x << 16, x is the mask
 * x << 4 , x is the starting position
 * | x, is the dword
 */

#[repr(u32)]
pub enum EndpointDescriptorPart {
    /** FunctionAddress*/
    Fa = (0x7F << 10) | (0 << 4) | 0,
    /** Endpoint Number*/
    En = (0xF << 10) | (7 << 4) | 0,
    /** Direction*/
    D = (0x3 << 10) | (11 << 4) | 0,
    /** MaximumPacketSize*/
    Mps = (0x7FF << 10) | (16 << 4) | 0,
}
#[repr(u32)]
pub enum EndpointDescriptorBitPart {
    /** Speed*/
    S = (13 << 16) | 0,
    /** Skip*/
    K = (14 << 16) | 0,
    /** Format*/
    F = (15 << 16) | 0,

    /** Connector (Custom)*/
    Con = (27 << 16) | 0,

    /** Dummy (Custom). Used in the Control List and indicating that this is for a possible device*/
    Dum = (28 << 16) | 0,

    /** Halted*/
    H = (0 << 16) | 2,
    /** toggleCarry */
    C = (1 << 16) | 2,
}

impl OhciEndpointDescriptor {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            val: addr as *mut u32,
        };
    }

    pub fn copy_from(&mut self, ep: &OhciEndpointDescriptor) {
        unsafe {
            self.val
                .add(0)
                .write_volatile(ep.val.add(0).read_volatile());
            self.val
                .add(1)
                .write_volatile(ep.val.add(1).read_volatile());
            self.val
                .add(2)
                .write_volatile(ep.val.add(2).read_volatile());
            self.val
                .add(3)
                .write_volatile(ep.val.add(3).read_volatile());
        }
    }

    pub fn address(&self) -> u32 {
        return self.val as u32;
    }

    pub fn zero_out(&mut self) {
        unsafe {
            self.val.add(0).write_volatile(0);
            self.val.add(1).write_volatile(0);
            self.val.add(2).write_volatile(0);
            self.val.add(3).write_volatile(0);
        }
    }

    pub fn is_set(&self, bit_part: EndpointDescriptorBitPart) -> bool {
        let part_u32 = bit_part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return 1 == val >> (part_u32 >> 16) & 1;
    }

    pub fn set(&mut self, bit_part: EndpointDescriptorBitPart, val: bool) {
        let part_u32 = bit_part as u32;
        let mut prev_val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        prev_val &= !(1 << (part_u32 >> 16));
        prev_val |= (val as u32) << (part_u32 >> 16);
        unsafe {
            self.val
                .add((part_u32 & 0xF) as usize)
                .write_volatile(prev_val)
        }
    }

    pub fn get_part(&self, part: EndpointDescriptorPart) -> u32 {
        let part_u32 = part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: EndpointDescriptorPart, val: u32) {
        let part_u32 = part as u32;
        let mut prev_val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        prev_val &= !((part_u32 >> 10) << ((part_u32 >> 4) & 0x1F));
        prev_val |= (val & (part_u32 >> 10)) << ((part_u32 >> 4) & 0x1F);
        unsafe {
            self.val
                .add((part_u32 & 0xF) as usize)
                .write_volatile(prev_val)
        }
    }

    pub fn tail_p(&self) -> u32 {
        unsafe { self.val.add(1).read_volatile() }
    }
    pub fn head_p(&self) -> u32 {
        (unsafe { self.val.add(2).read_volatile() } >> 4) << 4
    }
    pub fn next_ed(&self) -> u32 {
        unsafe { self.val.add(3).read_volatile() }
    }

    pub fn write_tail_p(&mut self, val: u32) {
        unsafe { self.val.add(1).write_volatile(val) }
    }
    pub fn write_head_p(&mut self, val: u32) {
        let mut prev_val = unsafe { self.val.add(2).read_volatile() };
        unsafe { self.val.add(2).write_volatile(val | prev_val & 0x3) }
    }
    pub fn write_next_ed(&mut self, val: u32) {
        unsafe { self.val.add(3).write_volatile(val) }
    }

    /**
     * Get´s the last ep in the ep list
     * Returns Option::None, if self.next_ed() == 0
     */
    pub fn get_last_ep(&self) -> Option<OhciEndpointDescriptor> {
        if self.next_ed() == 0 {
            return Option::None;
        }
        let mut ret = OhciEndpointDescriptor::new(self.next_ed() as *mut c_void);
        while ret.next_ed() != 0 {
            ret = OhciEndpointDescriptor::new(ret.next_ed() as *mut c_void);
        }
        return Option::Some(ret);
    }
}
