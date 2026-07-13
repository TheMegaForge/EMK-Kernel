use crate::{
    aml::definitions::TermArgInt::Add,
    drivers::usb::{
        ohci::{
            data_structures::OhciControlPart, structures::endpoint::EndpointDescriptorBitPart::S,
        },
        standard_requests::UsbStandardDeviceRequest,
    },
    time::sleep,
    utils::{inb, ind, inw, outb, outd, outw, queue::Queue},
};

#[repr(u16)]
pub enum UhciUsbCmdBitPart {
    /** Run/Stop*/
    Rs = 0,
    /** Host Controller Reset*/
    HcReset = 1,
    /** Global Reset*/
    GReset = 2,
    /** Enter Global Suspend Mode*/
    Egsm = 3,
    /** Force Global Resume*/
    Fgr = 4,
    /** Software Debug*/
    SwDbg = 5,
    /** Configure Flag*/
    Cf = 6,
    /** Max Packet*/
    MaxP = 7,
}

pub struct UhciUsbCmd {
    ioaddr: u16,
}

impl UhciUsbCmd {
    pub fn new(ioaddr: u16) -> Self {
        return Self { ioaddr };
    }

    pub fn is_set(&self, what_to_check: UhciUsbCmdBitPart) -> bool {
        let val = unsafe { inw(self.ioaddr) };
        return 0 != (val & (1 << what_to_check as u16));
    }

    pub fn set(&mut self, what_to_set: UhciUsbCmdBitPart, val: bool) {
        let mut prev_val = unsafe { inw(self.ioaddr) };
        let set_u16 = what_to_set as u16;
        prev_val &= !(1 << set_u16);
        unsafe {
            outw(self.ioaddr, prev_val | ((val as u16) << set_u16));
        }
    }
}

#[repr(u16)]
pub enum UhciUsbStatusBitPart {
    /** Usb Interrupt*/
    UsbInt = 0,
    UsbErrorInterrupt = 1,
    ResumeDetect = 2,
    HostSystemError = 3,
    HostControllerProcessError = 4,
    HcHalted = 5,
}

pub struct UhciUsbSts {
    ioaddr: u16,
}

impl UhciUsbSts {
    pub fn new(ioaddr: u16) -> Self {
        return Self { ioaddr };
    }

    pub fn as_u16(&self) -> u16 {
        return unsafe { inw(self.ioaddr) };
    }

    pub fn is_set(&self, what_to_check: UhciUsbStatusBitPart) -> bool {
        let val = unsafe { inw(self.ioaddr) };
        return 0 != (val & (1 << what_to_check as u16));
    }

    pub fn set(&mut self, what_to_set: UhciUsbStatusBitPart, val: bool) {
        let mut prev_val = unsafe { inw(self.ioaddr) };
        let set_u16 = what_to_set as u16;
        prev_val &= !(1 << set_u16);
        unsafe {
            outw(self.ioaddr, prev_val | ((val as u16) << set_u16));
        }
    }
}

#[repr(u16)]
pub enum UhciUsbInterrupt {
    /** Timeout/CRC*/
    Timeout = 0,
    Resume = 1,
    /** IOC*/
    InterruptOnCompletion = 2,
    ShortPacket = 3,
}

pub struct UhciInterruptEnable {
    ioaddr: u16,
}

impl UhciInterruptEnable {
    pub fn new(ioaddr: u16) -> Self {
        return Self { ioaddr };
    }

    pub fn is_set(&self, what_to_check: UhciUsbInterrupt) -> bool {
        let val = unsafe { inw(self.ioaddr) };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: UhciUsbInterrupt, val: bool) {
        let mut prev_val = unsafe { inw(self.ioaddr) };
        let set_u16 = what_to_set as u16;
        prev_val &= !(1 << set_u16);
        unsafe {
            outw(self.ioaddr, prev_val | ((val as u16) << set_u16));
        }
    }
}
#[repr(u16)]
pub enum UhciPortStatusControlBitPart {
    CurrentConnectStatus = 0,
    ConnectStatusChange = 1,
    PortEnabledDisabled = 2,
    PortEnabledDisabledChange = 3,
    ResumeDetect = 6,
    LowSpeedDeviceAttached = 8,
    PortReset = 9,
    Suspend = 12,
}
#[repr(u16)]
pub enum UhciPortStatusPart {
    LineStatus = (0x03 << 8) | 4,
}

pub struct UhciUsbPortStatusControl {
    ioaddr: u16,
}

impl UhciUsbPortStatusControl {
    pub fn new(ioaddr: u16) -> Self {
        return Self { ioaddr };
    }

    pub fn reset(&mut self) {
        self.set(UhciPortStatusControlBitPart::PortReset, true);
        sleep(40);
        self.set(UhciPortStatusControlBitPart::PortReset, false);
        self.set(UhciPortStatusControlBitPart::PortEnabledDisabled, true);
    }

    pub fn is_set(&self, what_to_check: UhciPortStatusControlBitPart) -> bool {
        let val = unsafe { inw(self.ioaddr) };
        return 0 != (val & (1 << what_to_check as u16));
    }

    pub fn set(&mut self, what_to_set: UhciPortStatusControlBitPart, val: bool) {
        let mut prev_val = unsafe { inw(self.ioaddr) };
        let set_u16 = what_to_set as u16;
        prev_val &= !(1 << set_u16);
        unsafe {
            outw(self.ioaddr, prev_val | ((val as u16) << set_u16));
        }
    }

    pub fn get(&self, what_to_get: UhciPortStatusPart) -> u16 {
        let val = unsafe { inw(self.ioaddr) };
        let get_u16 = what_to_get as u16;
        return (val >> (get_u16 & 0xFF)) & (get_u16 >> 8);
    }

    pub fn set_part(&self, what_to_get: UhciPortStatusPart, val: u16) {
        let mut prev_val = unsafe { inw(self.ioaddr) };
        let set_u16 = what_to_get as u16;
        prev_val &= !((set_u16 >> 8) << (set_u16 & 0xFF));
        prev_val |= (val & (set_u16 >> 8)) << (set_u16 & 0xFF);
        unsafe { outw(self.ioaddr, prev_val) };
    }

    pub fn is_reserved_1_set(&self) -> bool {
        return 1 == 1 & (unsafe { inw(self.ioaddr) } >> 7);
    }
}

pub struct UhciBar {
    ioaddr: u16,
}

impl UhciBar {
    pub const fn new(ioaddr: u16) -> Self {
        return Self { ioaddr };
    }

    pub fn usbcmd(&self) -> UhciUsbCmd {
        return UhciUsbCmd::new(self.ioaddr);
    }
    pub fn usbsts(&self) -> UhciUsbSts {
        return UhciUsbSts::new(self.ioaddr + 2);
    }
    pub fn usbinterruptenable(&self) -> UhciInterruptEnable {
        return UhciInterruptEnable::new(self.ioaddr + 4);
    }
    pub fn frame_number(&self) -> u16 {
        return unsafe { inw(self.ioaddr + 6) };
    }
    pub fn frame_list_base_address(&self) -> u32 {
        return unsafe { ind(self.ioaddr + 8) };
    }
    pub fn write_frame_list_base_address(&self, addr: u32) {
        unsafe { outd(self.ioaddr + 8, addr) }
    }
    pub fn sofmod(&self) -> u8 {
        return unsafe { inb(self.ioaddr + 0xC) };
    }
    pub fn port(&self, port: u8) -> UhciUsbPortStatusControl {
        return UhciUsbPortStatusControl::new(self.ioaddr + 0x10 + ((port as u16) - 1) * 2);
    }
}
#[repr(u32)]
pub enum UhciTransferDescriptorPart {
    /** Actual Length*/
    ActLen = (0x7FF << 10) | (0 << 4) | 1,
    Status = (0xFF << 10) | (16 << 4) | 1,
    CErr = (0x3 << 10) | (27 << 4) | 1,
    /** Packet Identification*/
    Pid = (0xFF << 10) | (0 << 4) | 2,
    DeviceAddress = (0x7F << 10) | (8 << 4) | 2,
    /** Endpoint */
    EndPt = (0xF << 10) | (15 << 4) | 2,
    /** Maximum Length*/
    MaxLen = (0x7FF << 10) | (21 << 4) | 2,
}

pub enum UhciTransferDescriptorBitPart {
    /** Terminate*/
    T = (0 << 16) | 0,
    /** QH/TD Select*/
    Q = (1 << 16) | 0,
    /** Depth/Breadth Select*/
    Vf = (2 << 16) | 0,
    /** Interrupt On Completion*/
    Ioc = (24 << 16) | 1,
    /** Isochronous Select*/
    Ios = (25 << 16) | 1,
    /** Low Speed Device*/
    Ls = (26 << 16) | 1,
    /** Short Packet Detect*/
    Spd = (29 << 16) | 1,
    /** Data Toggle*/
    D = (19 << 16) | 2,
}
#[repr(C)]
pub struct RawUhciTransferDescriptor {
    _dword0: u32,
    _dword1: u32,
    _dword2: u32,
    _dword3: u32,
}

impl RawUhciTransferDescriptor {
    #[inline(always)]
    pub fn wrapped(&self) -> UhciTransferDescriptor {
        return UhciTransferDescriptor {
            val: (&raw const *self) as *mut u32,
        };
    }
    #[inline(always)]
    pub fn address(&self) -> u32 {
        (&raw const *self).addr() as u32
    }
}

pub struct UhciTransferDescriptor {
    val: *mut u32,
}

impl UhciTransferDescriptor {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }
    pub fn from_raw(raw: *mut RawUhciTransferDescriptor) -> Self {
        return Self {
            val: raw as *mut u32,
        };
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

    pub fn is_set(&self, bit_part: UhciTransferDescriptorBitPart) -> bool {
        let part_u32 = bit_part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return 1 == val >> (part_u32 >> 16) & 1;
    }

    pub fn set(&mut self, bit_part: UhciTransferDescriptorBitPart, val: bool) {
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

    pub fn get_part(&self, part: UhciTransferDescriptorPart) -> u32 {
        let part_u32 = part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: UhciTransferDescriptorPart, val: u32) {
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

    pub fn link_pointer(&self) -> u32 {
        return (unsafe { self.val.read_volatile() } >> 4) << 4;
    }

    pub fn write_link_pointer(&mut self, mut val: u32) {
        let prev_val = unsafe { self.val.read_volatile() } & 0xF;
        val &= !0xF;
        unsafe { self.val.write_volatile(prev_val | val) };
    }

    pub fn buffer_pointer(&self) -> u32 {
        return unsafe { self.val.add(3).read_volatile() };
    }
    pub fn write_buffer_pointer(&mut self, val: u32) {
        unsafe { self.val.add(3).write_volatile(val) }
    }

    pub fn refresh_for_setup(&mut self, request: *const UsbStandardDeviceRequest) {
        self.set_part(UhciTransferDescriptorPart::Status, 0); // deactivates and clears error
        self.set_part(UhciTransferDescriptorPart::Pid, 0x2D);
        self.set_part(UhciTransferDescriptorPart::CErr, 0b11);
        self.set_part(UhciTransferDescriptorPart::MaxLen, 7);
        self.set_part(UhciTransferDescriptorPart::ActLen, 0);
        self.set(UhciTransferDescriptorBitPart::Ioc, false);
        self.set(UhciTransferDescriptorBitPart::D, false);
        self.write_buffer_pointer(request as u32);
    }

    pub fn refresh_for_in(&mut self, max_len: u16, ioc: bool, toggle: bool, active: bool) {
        self.set_part(UhciTransferDescriptorPart::Status, (1 << 7) * active as u32);
        self.set_part(UhciTransferDescriptorPart::Pid, 0x69);
        self.set_part(UhciTransferDescriptorPart::CErr, 0b11);
        self.set_part(UhciTransferDescriptorPart::MaxLen, max_len as u32);
        self.set_part(UhciTransferDescriptorPart::ActLen, 0);
        self.set(UhciTransferDescriptorBitPart::Ioc, ioc);
        self.set(UhciTransferDescriptorBitPart::D, toggle);
    }

    pub fn refresh_for_out(&mut self, max_len: u16, ioc: bool, toggle: bool, active: bool) {
        self.set_part(UhciTransferDescriptorPart::Status, (1 << 7) * active as u32);
        self.set_part(UhciTransferDescriptorPart::Pid, 0xE1);
        self.set_part(UhciTransferDescriptorPart::CErr, 0b11);
        self.set_part(UhciTransferDescriptorPart::MaxLen, max_len as u32);
        self.set_part(UhciTransferDescriptorPart::ActLen, 0);
        self.set(UhciTransferDescriptorBitPart::Ioc, ioc);
        self.set(UhciTransferDescriptorBitPart::D, toggle);
    }

    pub fn refresh_for_status(&mut self, r#in: bool) {
        self.set_part(UhciTransferDescriptorPart::Status, 1 << 7);
        if r#in {
            self.set_part(UhciTransferDescriptorPart::Pid, 0x69);
        } else {
            self.set_part(UhciTransferDescriptorPart::Pid, 0xE1);
        }
        self.set_part(UhciTransferDescriptorPart::CErr, 0b11);
        self.set_part(UhciTransferDescriptorPart::MaxLen, 0x7FF);
        self.set_part(UhciTransferDescriptorPart::ActLen, 0);
        self.set(UhciTransferDescriptorBitPart::Ioc, true);
        self.set(UhciTransferDescriptorBitPart::D, true);
    }

    pub fn link_next_td(&mut self, td_address: u32, vf: bool) {
        unsafe {
            self.val
                .write_volatile((td_address >> 4) << 4 | (vf as u32) << 2)
        };
    }
}
#[repr(C)]
pub struct RawUhciQueueHead {
    _dword0: u32,
    _dword1: u32,
    _alignment0: u32,
    _alignment1: u32,
}

impl RawUhciQueueHead {
    #[inline(always)]
    pub fn wrapped(&self) -> UhciQueueHead {
        return UhciQueueHead::new(&raw const *self as *mut u32);
    }
    pub fn empty() -> Self {
        return Self {
            _dword0: 0,
            _dword1: 0,
            _alignment0: 0,
            _alignment1: 0,
        };
    }
    pub fn with_data(dw0: u32, dw1: u32) -> Self {
        return Self {
            _dword0: dw0,
            _dword1: dw1,
            _alignment0: 0,
            _alignment1: 0,
        };
    }
}

pub enum UhciQueuePointer {
    Queue(*mut u32),
    Td(*mut u32),
}

impl UhciQueuePointer {
    #[inline(always)]
    pub fn address(&self) -> *mut u32 {
        return match self {
            Self::Queue(addr) => *addr,
            Self::Td(addr) => *addr,
        };
    }
}
#[derive(Clone, Copy)]
pub struct UhciQueueHead {
    val: *mut u32,
}

impl UhciQueueHead {
    pub const fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn last_head(&self) -> UhciQueuePointer {
        let mut is_queue = true;
        let mut current = self.val;
        let mut previous = self.val;
        loop {
            if (unsafe { current.read_volatile() }) & 1 == 1 {
                break;
            }
            previous = current;
            let val = unsafe { current.read_volatile() };
            is_queue = 2 == val & 2;
            current = (val & !0xF) as *mut u32;
        }
        if is_queue {
            return UhciQueuePointer::Queue(previous);
        } else {
            return UhciQueuePointer::Td(previous);
        }
    }

    pub fn last_element(&self) -> UhciQueuePointer {
        let mut is_queue = true;
        let mut current = self.val;
        let mut previous = self.val;
        loop {
            if is_queue {
                if unsafe { current.add(1).read_volatile() } & 1 == 1 {
                    break;
                }
            } else {
                if unsafe { current.read_volatile() } & 1 == 1 {
                    break;
                }
            }

            previous = current;
            let val;
            if is_queue {
                val = unsafe { current.add(1).read_volatile() };
            } else {
                val = unsafe { current.read_volatile() };
            }
            is_queue = 2 == val & 2;
            current = (val & !0xF) as *mut u32;
        }
        if is_queue {
            return UhciQueuePointer::Queue(previous);
        } else {
            return UhciQueuePointer::Td(previous);
        }
    }

    pub fn address(&self) -> u32 {
        return self.val as u32;
    }

    pub fn zero_out(&mut self) {
        unsafe {
            self.val.add(0).write_volatile(0);
            self.val.add(1).write_volatile(0);
        }
    }

    pub fn terminate(&mut self, terminate_head: bool, terminate_element: bool) {
        let head_prev = unsafe { self.val.read_volatile() } & !1;
        let element_prev = unsafe { self.val.add(1).read_volatile() } & !1;

        unsafe {
            self.val.write_volatile(head_prev | terminate_head as u32);
            self.val
                .add(1)
                .write_volatile(element_prev | terminate_element as u32);
        }
    }

    pub fn queue_head_link_t(&self) -> bool {
        return 1 == unsafe { self.val.add(0).read_volatile() & 1 };
    }
    pub fn queue_head_link_q(&self) -> bool {
        return 0 != unsafe { self.val.add(0).read_volatile() & 0b10 };
    }
    pub fn queue_head_link_pointer(&self) -> u32 {
        return (unsafe { self.val.add(0).read_volatile() } >> 4) << 4;
    }

    pub fn set_queue_head_link_t(&mut self, val: bool) {
        let prev_val = unsafe { self.val.add(0).read_volatile() } & !1;
        unsafe { self.val.add(0).write_volatile(prev_val | (val as u32)) };
    }

    pub fn set_queue_head_link_q(&mut self, val: bool) {
        let prev_val = unsafe { self.val.add(0).read_volatile() } & !0b10;
        unsafe { self.val.add(0).write_volatile(prev_val | (val as u32) << 1) };
    }

    pub fn set_queue_head_link_pointer(&mut self, mut val: u32) {
        val &= !0xF;
        let prev_val = unsafe { self.val.add(0).read_volatile() } & 0xF;
        unsafe { self.val.add(0).write_volatile(val | prev_val) };
    }

    pub fn queue_element_link_t(&self) -> bool {
        return 1 == unsafe { self.val.add(1).read_volatile() & 1 };
    }
    pub fn queue_element_link_q(&self) -> bool {
        return 0b10 == unsafe { self.val.add(1).read_volatile() & 1 };
    }
    pub fn queue_element_link_r(&self) -> bool {
        return 0b100 == unsafe { self.val.add(1).read_volatile() & 1 };
    }

    pub fn queue_element_link_pointer(&self) -> u32 {
        return unsafe { self.val.add(1).read_volatile() } & !0xF;
    }

    pub fn set_queue_element_link_t(&mut self, val: bool) {
        let prev_val = unsafe { self.val.add(1).read_volatile() } & !1;
        unsafe { self.val.add(1).write_volatile(prev_val | (val as u32)) };
    }
    pub fn set_queue_element_link_q(&mut self, val: bool) {
        let prev_val = unsafe { self.val.add(1).read_volatile() } & !0b10;
        unsafe { self.val.add(1).write_volatile(prev_val | (val as u32) << 1) };
    }
    pub fn set_queue_element_link_r(&mut self, val: bool) {
        let prev_val = unsafe { self.val.add(1).read_volatile() } & !0b100;
        unsafe { self.val.add(1).write_volatile(prev_val | (val as u32) << 2) };
    }
    pub fn set_queue_element_link_pointer(&mut self, mut val: u32) {
        val &= !0xF;
        let prev_val = unsafe { self.val.add(1).read_volatile() } & 0xF;
        unsafe { self.val.add(1).write_volatile(prev_val | val) };
    }
}
