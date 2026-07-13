use core::{ffi::c_void, ptr::null_mut};

pub struct XhciHcsParams1 {
    addr: *mut u32,
}
#[repr(u32)]
pub enum XhciHcsParams1Part {
    /** Number of Device Slots */
    MaxSlots = (0xFF << 16) | 0,
    /** Number Of Interrupters */
    MaxIntrs = (0x7FF << 16) | 8,
    /** Number of Ports*/
    MaxPorts = (0xFF << 16) | 24,
}

impl XhciHcsParams1 {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn get(&self, what_to_get: XhciHcsParams1Part) -> u32 {
        let val = unsafe { self.addr.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }
}

pub struct XhciHcsParams2 {
    addr: *mut u32,
}
#[repr(u32)]
pub enum XhciHcsParams2Part {
    /** Isochronous Scheduling Threshold */
    Ist = (0xF << 16) | 0,
    /** Event Ring Segment Table Max*/
    ErstMax = (0xF << 16) | 4,
}

impl XhciHcsParams2 {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn get(&self, what_to_get: XhciHcsParams2Part) -> u32 {
        let val = unsafe { self.addr.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }
    /** Scratchpad Restore*/
    pub fn spr(&self) -> bool {
        return 0 != unsafe { self.addr.read_volatile() } & 1 << 26;
    }
    /** Max Scratchpad Buffers*/
    pub fn max_scratchpad_bufs(&self) -> u16 {
        let val = unsafe { self.addr.read_volatile() } & ((0x1F << 21) | (0x1F << 27));
        let low = (val >> 27) & 0x1F;
        let high = (val >> 21) & 0x1F;
        return low as u16 | (high as u16) << 5;
    }
}
pub struct XhciHcsParams3 {
    addr: *mut u32,
}
#[repr(u32)]
pub enum XhciHcsParams3Part {
    U1DeviceExitLatency = (0xFF << 16) | 0,
    U2DeviceExitLatency = (0xFFFF << 16) | 16,
}

impl XhciHcsParams3 {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn get(&self, what_to_get: XhciHcsParams3Part) -> u32 {
        let val = unsafe { self.addr.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }
}

pub struct XhciHccParams1 {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciHccParams1Part {
    /** Maximum Primary Stream Array Size*/
    MaxPSASize = (0xF << 16) | 12,
    /** xHCI Extended Capabilities Pointer*/
    XEcp = (0xFFFF << 16) | 16,
}

#[repr(u32)]
pub enum XhciHccParams1BitPart {
    /** 64-bit Addressing Capability*/
    Ac64 = 0,
    /** BW Negotiation Capability*/
    Bnc = 1,
    /** Context Size*/
    Csz = 2,
    /** Port Power Control*/
    Ppc = 3,
    /** Port Indicators*/
    Pind = 4,
    /** Light HC Reset Capability*/
    Lhrc = 5,
    /** Latency Tolerance Messaging Capability*/
    Ltc = 6,
    /** No Secondary SID Support*/
    Nss = 7,
    /** Parse All Event Data*/
    Pae = 8,
    /** Stopped - Short Packet Capability*/
    Spc = 9,
    /** Stopped EDTLA Capability */
    Sec = 10,
    /** Contiguous Frame ID Capability  */
    Cfc = 11,
}

impl XhciHccParams1 {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn get(&self, what_to_get: XhciHccParams1Part) -> u32 {
        let val = unsafe { self.addr.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn is_set(&self, what_to_check: XhciHccParams1BitPart) -> bool {
        let val = unsafe { self.addr.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }
}

pub struct XhciHccParams2 {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciHccParams2BitPart {
    /** U3 Entry Capability*/
    U3c = 0,
    /** Configure Endpoint Command Max Exit Latency Too Large Capability*/
    Cmc = 1,
    /** Force Save Context Capability*/
    Fsc = 2,
    /** Compliance Transition Capability*/
    Ctc = 3,
    /** Large ESIT Payload Capability*/
    Lec = 4,
    /** Configuration Information Capability*/
    Cic = 5,
    /** Extended TBC Capability*/
    Etc = 6,
    /** Extended TBC TRB Status Capability*/
    Tsc = 7,
    /** Get/Set Extended Property Capability*/
    Gsc = 8,
    /** Virtualization Based Trusted I/O Capability*/
    Vtc = 9,
}

impl XhciHccParams2 {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn is_set(&self, what_to_check: XhciHccParams2BitPart) -> bool {
        let val = unsafe { self.addr.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }
}

pub struct XhciUsbCmd {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciUsbCmdBitPart {
    /** Run/Stop */
    Rs = 0,
    /** Host Controller Reset*/
    HcRst = 1,
    /** Interrupter Enable*/
    Inte = 2,
    /** Host System Error Enable*/
    Hsee = 3,
    /** Light Host Controller Reset*/
    LhcRst = 7,
    /** Controller Save State*/
    Css = 8,
    /** Controller Restore State*/
    Crs = 9,
    /** Enable Wrap Evet*/
    Ewe = 10,
    /** Enable U3 MFINDEX Stop*/
    Eu3s = 11,
    /** CEM Enable*/
    Cme = 13,
    /** Extended TBC Enable*/
    Ete = 14,
    /** Extended TBC TRB Status Enable*/
    TscEn = 15,
    /** VTIO Enable*/
    VtioEn = 16,
}

impl XhciUsbCmd {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn is_set(&self, what_to_check: XhciUsbCmdBitPart) -> bool {
        let val = unsafe { self.addr.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: XhciUsbCmdBitPart, val: bool) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe {
            self.addr
                .write_volatile(prev_val | ((val as u32) << set_u32))
        };
    }
}

pub struct XhciUsbSts {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciUsbStsBitPart {
    /** HCHalted*/
    HcH = 0,
    /** Host System Error*/
    Hse = 2,
    /** Event Interrupt*/
    Eint = 3,
    /** Port Change Detected*/
    Pcd = 4,
    /** Save State Status*/
    Sss = 8,
    /** Restore State Status*/
    Rss = 9,
    /** Save/Restore Error*/
    Sre = 10,
    /** Controller Not Ready*/
    Cnr = 11,
    /** Host Controller Error*/
    Hce = 12,
}

impl XhciUsbSts {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn error_present(&self) -> bool {
        return self.is_set(XhciUsbStsBitPart::Hce)
            || self.is_set(XhciUsbStsBitPart::Hse)
            || self.is_set(XhciUsbStsBitPart::Sre);
    }

    pub fn is_set(&self, what_to_check: XhciUsbStsBitPart) -> bool {
        let val = unsafe { self.addr.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: XhciUsbStsBitPart, val: bool) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe {
            self.addr
                .write_volatile(prev_val | ((val as u32) << set_u32))
        };
    }
}

pub struct XhciCrCr {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciCrCrBitPart {
    /** Ring Cycle State*/
    Rcs = 0,
    /** Command Stop*/
    Cs = 1,
    /** Command Abort*/
    Ca = 2,
    /** Command Ring Running*/
    Crr = 3,
}

impl XhciCrCr {
    pub const fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn is_set(&self, what_to_check: XhciCrCrBitPart) -> bool {
        let val = unsafe { self.addr.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: XhciCrCrBitPart, val: bool) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe {
            self.addr
                .write_volatile(prev_val | ((val as u32) << set_u32))
        };
    }

    pub fn write_command_ring_pointer(&mut self, mut pointer: u64) {
        pointer &= !0x3F;
        let prev_val = unsafe { self.addr.read_volatile() } & 0x3F;
        unsafe {
            self.addr
                .write_volatile(prev_val | (pointer & 0xFFFFFFFF) as u32)
        }
        unsafe { self.addr.add(1).write_volatile((pointer >> 32) as u32) };
    }
}

pub struct XhciConfig {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciConfigBitPart {
    /** U3 Entry Enable*/
    U3e = 8,
    /** Configuration Information Enable*/
    Cie = 9,
}

impl XhciConfig {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn is_set(&self, what_to_check: XhciConfigBitPart) -> bool {
        let val = unsafe { self.addr.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: XhciConfigBitPart, val: bool) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe {
            self.addr
                .write_volatile(prev_val | ((val as u32) << set_u32))
        };
    }

    pub fn set_max_slots_en(&mut self, val: u8) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        prev_val &= !0xFF;
        unsafe { self.addr.write_volatile(prev_val | val as u32) };
    }
}

pub struct XhciPortSc {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciPortScBitPart {
    /** Current Connect Status*/
    Ccs = 0,
    /** Port Enabled/Disabled */
    Ped = 1,
    /** Over-current Active*/
    Oca = 3,
    /** Port Reset*/
    Pr = 4,
    /** Port Power*/
    Pp = 9,
    /** Port Link State Write Strobe*/
    Lws = 16,
    /** Connect Status Change*/
    Csc = 17,
    /** Port Enabled/Disabled Change*/
    Pec = 18,
    /** Warm Port Reset Change*/
    Wrc = 19,
    /** Over-current Change*/
    Occ = 20,
    /** Port Reset Change*/
    Prc = 21,
    /** Port Link State Change*/
    Plc = 22,
    /** Port Config Error Change*/
    Cec = 23,
    /** Cold Attach Status*/
    Cas = 24,
    /** Wake on Connect Enable*/
    Wce = 25,
    /** Wake on Disconnect Enable*/
    Wde = 26,
    /** Wake on Over-current Enable*/
    Woe = 27,
    /** Device Removable*/
    Dr = 30,
    /** Warm Port Reset*/
    Wpr = 31,
}
#[repr(u32)]
pub enum XhciPortScPart {
    /** Port Link State*/
    Pls = (0xF << 16) | 5,
    PortSpeed = (0xF << 16) | 10,
    /** Port Indicator Control*/
    Pic = (0x3 << 16) | 14,
}

impl XhciPortSc {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn is_set(&self, what_to_check: XhciPortScBitPart) -> bool {
        let val = unsafe { self.addr.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: XhciPortScBitPart, val: bool) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe {
            self.addr
                .write_volatile(prev_val | ((val as u32) << set_u32));
        }
    }

    pub fn get(&self, what_to_get: XhciPortScPart) -> u32 {
        let val = unsafe { self.addr.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: XhciPortScPart, val: u32) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.addr.write_volatile(prev_val) }
    }
}

pub struct XhciPortPmscUsb3 {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciPortPmscUsb3Part {
    U1Timeout = (0xFF << 16) | 0,
    U2Timeout = (0xFF << 16) | 8,
}

impl XhciPortPmscUsb3 {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn get(&self, what_to_get: XhciPortPmscUsb3Part) -> u32 {
        let val = unsafe { self.addr.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: XhciPortPmscUsb3Part, val: u32) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.addr.write_volatile(prev_val) }
    }
    /** Force Link PM Accept*/
    pub fn fla(&self) -> bool {
        return 0 != (unsafe { self.addr.read_volatile() } >> 16) & 1;
    }
    pub fn set_fla(&mut self, fla: bool) {
        let prev_val = unsafe { self.addr.read_volatile() } & !(1 << 16);
        unsafe { self.addr.write_volatile(prev_val | (fla as u32) << 16) };
    }
}

pub struct XhciPortPmscUsb2 {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciPortPmscUsb2BitPart {
    /** Remote Wakeup Enable*/
    Rwe = 3,
    /** Hardware LPM Enable*/
    Hle = 16,
}

#[repr(u32)]
pub enum XhciPortPmscUsb2Part {
    /** L1 Status*/
    L1S = (0x7 << 16) | 0,
    /** Best Effort Service Latency*/
    Besl = (0xF << 16) | 4,
    L1DeviceSlot = (0xFF << 16) | 8,
    /** Port Test Control*/
    TestMode = (0xF << 16) | 28,
}

impl XhciPortPmscUsb2 {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn get(&self, what_to_get: XhciPortPmscUsb2Part) -> u32 {
        let val = unsafe { self.addr.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: XhciPortPmscUsb2Part, val: u32) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.addr.write_volatile(prev_val) }
    }

    pub fn is_set(&self, what_to_check: XhciPortPmscUsb2BitPart) -> bool {
        let val = unsafe { self.addr.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: XhciPortPmscUsb2BitPart, val: bool) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe {
            self.addr
                .write_volatile(prev_val | ((val as u32) << set_u32));
        }
    }
}

pub struct XhciPortLiUsb3 {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciPortLiUsb3Part {
    LinkErrorCount = (0xFFFF << 16) | 0,
    /** Rx Lane Count*/
    Rlc = (0xF << 16) | 16,
    /** Tx Lane Count*/
    Tlc = (0xF << 16) | 20,
}

impl XhciPortLiUsb3 {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn get(&self, what_to_get: XhciPortLiUsb3Part) -> u32 {
        let val = unsafe { self.addr.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }
}

pub struct XhciPortHlpMcUsb2 {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciPortHlpMcUsb2Part {
    /** Host Initiated Resume Duration Mode*/
    Hirdm = (0x3 << 16) | 0,
    L1Timeout = (0xFF << 16) | 2,
    /** Best Effort Service Latency Deep*/
    Besld = (0xF << 16) | 10,
}

impl XhciPortHlpMcUsb2 {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn get(&self, what_to_get: XhciPortHlpMcUsb2Part) -> u32 {
        let val = unsafe { self.addr.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: XhciPortHlpMcUsb2Part, val: u32) {
        let mut prev_val = unsafe { self.addr.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.addr.write_volatile(prev_val) }
    }
}

pub struct XhciInterruptRegisterSet {
    addr: *mut u32,
}

#[repr(u32)]
pub enum XhciInterruptRegisterSetPart {
    /** Interrupt Moderation Interval*/
    ImodI = (0xFFFF << 10) | (0 << 4) | 1,
    /** Interrupt Moderation Counter*/
    ImodC = (0xFFFF << 10) | (16 << 4) | 1,
    EventRingSegmentTableSize = (0xFFFF << 10) | (0 << 4) | 2,
    /** Dequeue ERST Segment Index*/
    Desi = (0x7 << 10) | (0 << 4) | 6,
}

pub enum XhciInterruptRegisterSetBitPart {
    Ip = (0 << 16) | 0,
    Ie = (1 << 16) | 0,
    Ehb = (3 << 16) | 6,
}

impl XhciInterruptRegisterSet {
    pub const fn new(addr: *mut c_void) -> XhciInterruptRegisterSet {
        return Self {
            addr: addr as *mut u32,
        };
    }
    pub fn is_set(&self, bit_part: XhciInterruptRegisterSetBitPart) -> bool {
        let part_u32 = bit_part as u32;
        let val = unsafe { self.addr.add((part_u32 & 0xF) as usize).read_volatile() };
        return 1 == val >> (part_u32 >> 16) & 1;
    }

    pub fn set(&mut self, bit_part: XhciInterruptRegisterSetBitPart, val: bool) {
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

    pub fn get_part(&self, part: XhciInterruptRegisterSetPart) -> u32 {
        let part_u32 = part as u32;
        let val = unsafe { self.addr.add((part_u32 & 0xF) as usize).read_volatile() };
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: XhciInterruptRegisterSetPart, val: u32) {
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
    pub fn set_event_ring_table_base_address(&mut self, mut base: u64) {
        base &= !0x1F;
        unsafe {
            self.addr.add(4).write_volatile((base & 0xFFFFFFFF) as u32);
            self.addr.add(5).write_volatile((base >> 32) as u32);
        }
    }
    pub fn set_event_ring_dequeue_pointer(&mut self, mut base: u64) {
        base &= !0xF;
        unsafe {
            let prev_val = self.addr.add(6).read_volatile() & 0xF;
            self.addr
                .add(6)
                .write_volatile(prev_val | (base & 0xFFFFFFFF) as u32);
            self.addr.add(7).write_volatile((base >> 32) as u32);
        }
    }
}

pub struct XhciDoorbell {
    addr: *mut u32,
}

pub const XHCI_DB_TARGET_CONTROL_EP0: u8 = 0;
pub fn db_target_for_ep(ep_index: u8, r#in: bool) -> u8 {
    (ep_index * 2) + r#in as u8
}

impl XhciDoorbell {
    pub const fn new(addr: *mut c_void) -> Self {
        return Self {
            addr: addr as *mut u32,
        };
    }

    pub fn ring(&self, db_target: u8, db_stream_id: u16) {
        unsafe {
            self.addr
                .write_volatile(db_target as u32 | (db_stream_id as u32) << 16)
        };
    }
}

pub struct XhciBar {
    capability_base: *mut c_void,
    operational_base: *mut c_void,
    runtime_base: *mut c_void,
}

impl XhciBar {
    pub const fn new(capability_base: *mut c_void) -> Self {
        return Self {
            capability_base,
            operational_base: null_mut(),
            runtime_base: null_mut(),
        };
    }
    #[inline(always)]
    pub fn get_base(&self) -> *mut c_void {
        return self.capability_base;
    }

    #[inline(always)]
    pub fn set_operational_base(&mut self) {
        self.operational_base = unsafe { self.capability_base.add(self.caplength() as usize) };
    }
    #[inline(always)]
    pub fn set_runtime_base(&mut self) {
        self.runtime_base = unsafe { self.capability_base.add(self.rtsoff() as usize) }
    }

    pub fn caplength(&self) -> u8 {
        return unsafe { (self.capability_base as *mut u8).read_volatile() };
    }
    pub fn hciversion(&self) -> u16 {
        unsafe { (self.capability_base.add(1) as *mut u16).read_volatile() }
    }
    pub fn hcsparams1(&self) -> XhciHcsParams1 {
        return XhciHcsParams1::new(unsafe { self.capability_base.add(4) });
    }
    pub fn hcsparams2(&self) -> XhciHcsParams2 {
        return XhciHcsParams2::new(unsafe { self.capability_base.add(8) });
    }
    pub fn hcsparams3(&self) -> XhciHcsParams3 {
        return XhciHcsParams3::new(unsafe { self.capability_base.add(0x0C) });
    }
    pub fn hccparams1(&self) -> XhciHccParams1 {
        return XhciHccParams1::new(unsafe { self.capability_base.add(0x10) });
    }
    pub fn dboff(&self) -> u32 {
        return unsafe { (self.capability_base.add(0x14) as *mut u32).read_volatile() };
    }
    pub fn rtsoff(&self) -> u32 {
        return unsafe { (self.capability_base.add(0x18) as *mut u32).read_volatile() };
    }
    pub fn hccparams2(&self) -> XhciHccParams2 {
        return XhciHccParams2::new(unsafe { self.capability_base.add(0x1C) });
    }
    pub fn vtiosoff(&self) -> u32 {
        return unsafe { (self.capability_base.add(0x20) as *mut u32).read_volatile() };
    }
    pub fn usbcmd(&self) -> XhciUsbCmd {
        return XhciUsbCmd::new(self.operational_base);
    }
    pub fn usbsts(&self) -> XhciUsbSts {
        return XhciUsbSts::new(unsafe { self.operational_base.add(4) });
    }

    pub fn is_page_size_valid(&self, mut page_size: u32) -> bool {
        page_size &= !0xFFF;
        let bit = page_size.ilog2() - 12;
        return 0
            != unsafe { (self.operational_base.add(0x8) as *mut u32).read_volatile() } & 1 << bit;
    }

    pub fn dnctrl_notification_enabled(&self, mut index: u8) -> bool {
        return 0
            != unsafe { (self.operational_base.add(0x14) as *mut u32).read_volatile() }
                & 1 << index;
    }
    pub fn dnctrl_set_notification(&self, mut index: u8, val: bool) {
        let mut prev_val = unsafe { (self.operational_base.add(0x14) as *mut u32).read_volatile() };
        prev_val &= !(1 << index);
        unsafe {
            (self.operational_base.add(0x14) as *mut u32)
                .write_volatile(prev_val | (val as u32) << index)
        };
    }
    pub fn crcr(&self) -> XhciCrCr {
        return XhciCrCr::new(unsafe { self.operational_base.add(0x18) });
    }
    pub fn write_dcbaap(&mut self, mut pointer: u64) {
        pointer &= !0x1F;
        unsafe {
            (self.operational_base.add(0x30) as *mut u32)
                .write_volatile((pointer & 0xFFFFFFFF) as u32)
        };
        unsafe {
            (self.operational_base.add(0x34) as *mut u32).write_volatile((pointer >> 32) as u32)
        };
    }
    pub fn config(&self) -> XhciConfig {
        return XhciConfig::new(unsafe { self.operational_base.add(0x38) });
    }
    pub fn portsc(&self, port: u8) -> XhciPortSc {
        let mut base = unsafe { self.operational_base.add(0x400) };
        base = unsafe { base.add(0x10 * (port - 1) as usize) };
        return XhciPortSc::new(base);
    }
    pub fn portpmsc_usb3(&self, port: u8) -> XhciPortPmscUsb3 {
        let mut base = unsafe { self.operational_base.add(0x404) };
        base = unsafe { base.add(0x10 * (port - 1) as usize) };
        return XhciPortPmscUsb3::new(base);
    }
    pub fn portpmsc_usb2(&self, port: u8) -> XhciPortPmscUsb2 {
        let mut base = unsafe { self.operational_base.add(0x404) };
        base = unsafe { base.add(0x10 * (port - 1) as usize) };
        return XhciPortPmscUsb2::new(base);
    }
    pub fn portli_usb3(&self, port: u8) -> XhciPortLiUsb3 {
        let mut base = unsafe { self.operational_base.add(0x408) };
        base = unsafe { base.add(0x10 * (port - 1) as usize) };
        return XhciPortLiUsb3::new(base);
    }
    pub fn porthlpmc_usb2(&self, port: u8) -> XhciPortHlpMcUsb2 {
        let mut base = unsafe { self.operational_base.add(0x40C) };
        base = unsafe { base.add(0x10 * (port - 1) as usize) };
        return XhciPortHlpMcUsb2::new(base);
    }
    pub fn microframe_index(&self) -> u16 {
        return (unsafe { (self.runtime_base as *mut u32).read_volatile() } & 0x3FFF) as u16;
    }
    pub fn ir(&self, index: u16) -> XhciInterruptRegisterSet {
        return XhciInterruptRegisterSet::new(unsafe {
            self.runtime_base.add(0x20 + index as usize * 0x20)
        });
    }
    pub fn doorbell(&self, index: u16) -> XhciDoorbell {
        return XhciDoorbell::new(unsafe {
            self.capability_base
                .add(self.dboff() as usize)
                .add(index as usize * 4)
        });
    }
}
