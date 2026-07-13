use crate::{
    drivers::usb::xhci::{data_structures::XhciTrbId, structures::endpoint::XhciEndpoint},
    utils::traits::AsU64,
};

pub mod command_ring;
pub mod contexts;
pub mod device;
pub mod endpoint;
pub mod interface;
pub mod interrupter;
pub const XHCI_SLOT_TYPE_GENERAL: u8 = 0;

pub enum XhciCommand {
    NoOp,
    EnableSlot {
        slot_type: u8,
    },
    DisableSlot {
        slot_id: u8,
    },
    /** bsr = Block Set Address Request*/
    AddressDevice {
        slot_id: u8,
        input_context_address: u64,
        bsr: bool,
    },
    /** dc = Deconfigure*/
    ConfigureEndpoint {
        slot_id: u8,
        input_context_address: u64,
        dc: bool,
    },
    EvaluateContext {
        slot_id: u8,
        input_context_address: u64,
    },
    /** tsp = Transfer State Preserve*/
    ResetEndpoint {
        slot_id: u8,
        endpoint_id: u8,
        tsp: bool,
    },
    /** sp = suspend*/
    StopEndpoint {
        slot_id: u8,
        endpoint_id: u8,
        /** suspend */
        sp: bool,
    },
    /** dcs = Dequeue Cycle State. sct = Stream Context Type*/
    SetTRDequeuePointer {
        slot_id: u8,
        endpoint_id: u8,
        stream_id: u16,
        new_tr_dequeue_pointer_address: u64,
        dcs: bool,
        sct: bool,
    },
    ResetDevice {
        slot_id: bool,
    },
    GetPortBandwidth {
        hub_slot_id: u8,
        dev_speed: u8,
        port_bandwidth_context_address: u64,
    },
    /** header_info_lo value must be nudged to the 0th bit and not the 4th*/
    ForceHeader {
        r#type: u8,
        root_hub_port_number: u8,
        header_info_lo: u32,
        header_info_mid: u32,
        header_info_hi: u32,
    },
    GetExtendedProperty {
        slot_id: u8,
        endpoint_id: u8,
        sub_type: u8,
        extended_capability_identifier: u16,
        extended_property_context_address: u64,
    },
    SetExtendedProperty {
        slot_id: u8,
        endpoint_id: u8,
        sub_type: u8,
        capability_parameter: u8,
        extended_capability_parameter: u16,
    },
}
#[repr(align(16))]
pub struct RawXhciTrb {
    _dword0: u32,
    _dword1: u32,
    _dword2: u32,
    _dword3: u32,
}

impl RawXhciTrb {
    #[inline(always)]
    pub fn as_ptr<T>(&self) -> *const T {
        return (&raw const *self) as *const T;
    }
    #[inline(always)]
    pub fn address(&self) -> u64 {
        (&raw const *self).as_u64()
    }
}

#[repr(u32)]
pub enum XhciLinkTrbPart {
    InterrupterTarget = (0x3FF << 10) | (22 << 4) | 2,
    TrbType = (0x3F << 10) | (10 << 4) | 3,
}
#[repr(u32)]
pub enum XhciLinkTrbBitPart {
    /** Interrupt On Completion*/
    Ioc = (5 << 16) | 3,
    /** Chain bit*/
    Ch = (4 << 16) | 3,
    /** Toggle Cycle*/
    Tc = (1 << 16) | 3,
    /** Cycle*/
    C = (0 << 16) | 3,
}

pub struct XhciLinkTrb {
    addr: *mut u32,
}

impl XhciLinkTrb {
    #[inline(always)]
    pub fn from<T>(r#in: *const T) -> Self {
        return Self {
            addr: r#in as *mut u32,
        };
    }

    pub fn zero_out(&mut self) {
        unsafe {
            self.addr.write_volatile(0);
            self.addr.add(1).write_volatile(0);
            self.addr.add(2).write_volatile(0);
            self.addr.add(3).write_volatile(0);
        }
    }

    #[inline(always)]
    pub fn address(&self) -> u64 {
        return self.addr.as_u64();
    }

    pub fn is_set(&self, bit_part: XhciLinkTrbBitPart) -> bool {
        let part_u32 = bit_part as u32;
        let val = unsafe { self.addr.add((part_u32 & 0xF) as usize).read_volatile() };
        return 1 == val >> (part_u32 >> 16) & 1;
    }

    pub fn set(&mut self, bit_part: XhciLinkTrbBitPart, val: bool) {
        let part_u32 = bit_part as u32;
        let mut prev_val = unsafe { self.addr.add((part_u32 & 0xF) as usize).read_volatile() };
        prev_val &= !(1 << (part_u32 >> 16));
        prev_val |= (val as u32) << (part_u32 >> 16);
        unsafe {
            self.addr
                .add((part_u32 & 0xF) as usize)
                .write_volatile(prev_val)
        }
    }

    pub fn get_part(&self, part: XhciLinkTrbPart) -> u32 {
        let part_u32 = part as u32;
        let val = unsafe { self.addr.add((part_u32 & 0xF) as usize).read_volatile() };
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: XhciLinkTrbPart, val: u32) {
        let part_u32 = part as u32;
        let mut prev_val = unsafe { self.addr.add((part_u32 & 0xF) as usize).read_volatile() };
        prev_val &= !((part_u32 >> 10) << ((part_u32 >> 4) & 0x1F));
        prev_val |= (val & (part_u32 >> 10)) << ((part_u32 >> 4) & 0x1F);
        unsafe {
            self.addr
                .add((part_u32 & 0xF) as usize)
                .write_volatile(prev_val)
        }
    }
    pub fn set_ring_segment_pointer(&mut self, mut addr: u64) {
        addr &= !0xF;
        unsafe {
            self.addr.write_volatile((addr & 0xFFFFFFFF) as u32);
            self.addr.add(1).write_volatile((addr >> 32) as u32);
        }
    }
    pub fn ring_segment_pointer(&self) -> u64 {
        unsafe {
            self.addr.read_volatile() as u64 | ((self.addr.add(1).read_volatile() as u64) << 32)
        }
    }
}

#[repr(u32)]
pub enum XhciCommandCompletionEventTrbPart {
    CompletionCode = (0xFF << 10) | (24 << 4) | 2,
    SlotId = (0xFF << 10) | (24 << 4) | 3,
    VfId = (0xFF << 10) | (16 << 4) | 3,
    TrbType = (0x3F << 10) | (10 << 4) | 3,
}

pub struct XhciCommandCompletionEventTrb {
    addr: *mut u32,
}

impl XhciCommandCompletionEventTrb {
    #[inline(always)]
    pub fn from<T>(r#in: *const T) -> Self {
        return Self {
            addr: r#in as *mut u32,
        };
    }

    pub fn zero_out(&mut self) {
        unsafe {
            self.addr.write_volatile(0);
            self.addr.add(1).write_volatile(0);
            self.addr.add(2).write_volatile(0);
            self.addr.add(3).write_volatile(0);
        }
    }

    #[inline(always)]
    pub fn address(&self) -> u64 {
        return self.addr.as_u64();
    }

    pub fn cycle(&self) -> bool {
        return 1 == unsafe { self.addr.add(3).read_volatile() } & 1;
    }

    pub fn get_part(&self, part: XhciCommandCompletionEventTrbPart) -> u32 {
        let part_u32 = part as u32;
        let val = unsafe { self.addr.add((part_u32 & 0xF) as usize).read_volatile() };
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: XhciCommandCompletionEventTrbPart, val: u32) {
        let part_u32 = part as u32;
        let mut prev_val = unsafe { self.addr.add((part_u32 & 0xF) as usize).read_volatile() };
        prev_val &= !((part_u32 >> 10) << ((part_u32 >> 4) & 0x1F));
        prev_val |= (val & (part_u32 >> 10)) << ((part_u32 >> 4) & 0x1F);
        unsafe {
            self.addr
                .add((part_u32 & 0xF) as usize)
                .write_volatile(prev_val)
        }
    }

    pub fn command_trb_pointer(&self) -> u64 {
        return unsafe {
            self.addr.read_volatile() as u64 | (self.addr.add(1).read_volatile() as u64) >> 32
        };
    }
    pub fn command_completion_parameter(&self) -> u32 {
        return unsafe { self.addr.add(2).read_volatile() & 0xFFFFFF };
    }
}
#[repr(C, packed)]
pub struct XhciSetupStageTrb {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
    dword2: u32,
    dword3: u32,
}
#[repr(u32)]
pub enum XhciTransferType {
    NoDataStage = 0,
    OutDataStage = 2,
    InDataStage = 3,
}

impl XhciSetupStageTrb {
    pub fn set_interrupter_target(&mut self, val: u16) {
        let prev_val = self.dword2 & !(0x3FF << 22);
        self.dword2 = prev_val | (val as u32) << 22;
    }

    pub fn set_trb_transfer_length(&mut self) {
        let prev_val = self.dword2 & !0x1FFFF;
        self.dword2 = prev_val | 8;
    }
    pub fn set_ioc(&mut self, val: bool) {
        let prev_val = self.dword3 & !(1 << 5);
        self.dword3 = prev_val | (val as u32) << 5;
    }
    pub fn set_idt(&mut self, val: bool) {
        let prev_val = self.dword3 & !(1 << 6);
        self.dword3 = prev_val | (val as u32) << 6;
    }
    pub fn set_type(&mut self) {
        let prev_val = self.dword3 & !(0x3F << 10);
        self.dword3 = prev_val | (XhciTrbId::Setup as u32) << 10;
    }
    pub fn set_trt(&mut self, val: XhciTransferType) {
        let prev_val = self.dword3 & !(0x3 << 16);
        self.dword3 = prev_val | (val as u32) << 16;
    }
    pub fn set_c(&mut self, val: bool) {
        let prev_val = self.dword3 & !1;
        self.dword3 = prev_val | val as u32;
    }
}
#[repr(u32)]
pub enum XhciDataStageTrbBitPart {
    /** Cycle*/
    C = 0,
    /** Evaluate Next TRB*/
    Ent = 1,
    /** Interrupt on Short Packet*/
    Isp = 2,
    /** No Snoop*/
    Ns = 3,
    /** Chain*/
    Ch = 4,
    /** Interrupt On Completion*/
    Ioc = 5,
    /** Immediate Data*/
    Idt = 6,
    /** Direction*/
    Dir = 16,
}

#[repr(C, packed)]
pub struct XhciDataStageTrb {
    pub data_buffer: u64,
    dword2: u32,
    dword3: u32,
}

impl XhciDataStageTrb {
    pub fn set_interrupter_target(&mut self, val: u16) {
        let prev_val = self.dword2 & !(0x3FF << 22);
        self.dword2 = prev_val | ((val & 0x3FF) as u32) << 22;
    }
    pub fn set_td_size(&mut self, val: u8) {
        let prev_val = self.dword2 & !(0x1F << 17);
        self.dword2 = prev_val | ((val & 0x1F) as u32) << 17;
    }
    pub fn trb_transfer_length(&mut self, val: u32) {
        let prev_val = self.dword2 & !0x1FFFF;
        self.dword2 = prev_val | val & 0x1FFFF;
    }
    pub fn set_type(&mut self) {
        let prev_val = self.dword3 & !(0x3F << 10);
        self.dword3 = prev_val | (XhciTrbId::Data as u32) << 10;
    }
    pub fn set(&mut self, what_to_set: XhciDataStageTrbBitPart, val: bool) {
        let mut prev_val = self.dword3;
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        self.dword3 = prev_val | ((val as u32) << set_u32);
    }
}
#[repr(u32)]
pub enum XhciStatusStageTrbBitPart {
    C = 0,
    Ent = 1,
    Ch = 4,
    Ioc = 5,
    Dir = 16,
}

#[repr(C, packed)]
pub struct XhciStatusStageTrb {
    _reserved0: u32,
    _reserved1: u32,
    dword2: u32,
    dword3: u32,
}

impl XhciStatusStageTrb {
    pub fn set_interrupter_target(&mut self, val: u16) {
        let prev_val = self.dword2 & !(0x3FF << 22);
        self.dword2 = prev_val | (val as u32) << 22;
    }
    pub fn set_type(&mut self) {
        let prev_val = self.dword3 & !(0x3F << 10);
        self.dword3 = prev_val | (XhciTrbId::Status as u32) << 10;
    }

    pub fn set(&mut self, what_to_set: XhciStatusStageTrbBitPart, val: bool) {
        let mut prev_val = self.dword3;
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        self.dword3 = prev_val | ((val as u32) << set_u32);
    }
}
#[repr(u32)]
pub enum XhciNormalTrbBitPart {
    /** Block Event Interrupt*/
    Bei = 9,
    /** Immediate Data*/
    Idt = 6,
    /** Interrupt On Completion*/
    Ioc = 5,
    /** Chain bit*/
    Ch = 4,
    /** No Snoop*/
    Ns = 3,
    /** Interrupt-on Short Packet*/
    Isp = 2,
    /** Evaluate Next TRB*/
    Ent = 1,
    /** Cycle bit*/
    C = 0,
}

#[repr(C, packed)]
pub struct RawXhciNormalTrb {
    pub data_buffer: u64,
    dword2: u32,
    dword3: u32,
}

impl RawXhciNormalTrb {
    pub fn from_mut<T>(r#in: *mut T) -> &'static mut Self {
        return unsafe { &mut *(r#in as *mut Self) };
    }
    pub fn from<T>(r#in: *const T) -> &'static Self {
        return unsafe { &*(r#in as *const Self) };
    }
    pub fn set_interrupter_target(&mut self, val: u16) {
        let prev_val = self.dword2 & !(0x3FF << 22);
        self.dword2 = prev_val | (val as u32) << 22;
    }
    pub fn set_td_size(&mut self, val: u8) {
        let prev_val = self.dword2 & !(0x1F << 17);
        self.dword2 = prev_val | (val as u32) << 17;
    }
    pub fn set_trb_transfer_length(&mut self, val: u32) {
        let prev_val = self.dword2 & !0x1FFFF;
        self.dword2 = prev_val | val & 0x1FFFF;
    }
    pub fn set_type(&mut self) {
        let prev_val = self.dword3 & !(0x3F << 10);
        self.dword3 = prev_val | (XhciTrbId::Normal as u32) << 10;
    }
    pub fn set(&mut self, what_to_set: XhciNormalTrbBitPart, val: bool) {
        let mut prev_val = self.dword3;
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        self.dword3 = prev_val | ((val as u32) << set_u32);
    }
    pub fn is_set(&self, what_to_check: XhciNormalTrbBitPart) -> bool {
        return 1 == self.dword3 >> (what_to_check as u32) & 1;
    }
}

pub struct XhciTransferEventTrb {
    ptr: *const u32,
}

impl XhciTransferEventTrb {
    pub fn from<T>(r#in: *const T) -> Self {
        return Self {
            ptr: r#in as *const u32,
        };
    }
    pub fn trb_pointer(&self) -> u64 {
        let lo = unsafe { self.ptr.read_volatile() };
        let hi = unsafe { self.ptr.read_volatile() };
        return lo as u64 | (hi as u64) << 32;
    }
    pub fn completion_code(&self) -> u8 {
        return ((unsafe { self.ptr.add(2).read_volatile() } >> 24) & 0xFF) as u8;
    }
    pub fn slot_id(&self) -> u8 {
        return ((unsafe { self.ptr.add(3).read_volatile() } >> 24) & 0xFF) as u8;
    }
    pub fn endpoint_id(&self) -> u8 {
        return unsafe { (self.ptr.add(3).read_volatile() >> 16) & 0x1F } as u8;
    }
}
