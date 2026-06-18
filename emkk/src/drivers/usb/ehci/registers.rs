use core::ptr::null_mut;

use crate::{drivers::usb::independent::HciState, hal::print::simple_kernel_panic};

pub struct HcsParams {
    val: *const u32,
}

type PortRoutingRule = u8;

#[repr(u32)]
pub enum HcsParamsPart {
    NPorts = (0xF << 16) | 0,
    Npcc = (0xF << 16) | 8,
    Ncc = (0xF << 16) | 12,
    DebugPortNumber = (0xF << 16) | 20,
}
#[repr(u32)]
pub enum HcsParamsBitPart {
    PortPowerControl = 4,
    PortRoutingRules = 7,
    PortIndicators = 16,
}

impl HcsParams {
    #[inline(always)]
    pub fn new(address: u64) -> HcsParams {
        return HcsParams {
            val: address as *const u32,
        };
    }

    pub fn get(&self, what_to_get: HcsParamsPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn is_set(&self, what_to_check: HcsParamsBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }
}

pub struct HccParams {
    val: *const u32,
}

#[repr(u32)]
pub enum HccParamsPart {
    IsochronousSchedulingThreshold = (0xF << 16) | 4,
    Eecp = (0xFF << 16) | 8,
}
#[repr(u32)]
pub enum HccParamsBitPart {
    QwordBitAddressingCapability = 0, /* QWord = 64 */
    ProgrammableFrameListFlag = 1,
    AsynchronousScheduleParkCapability = 2,
}

impl HccParams {
    #[inline(always)]
    pub fn new(address: u64) -> HccParams {
        return HccParams {
            val: address as *const u32,
        };
    }

    pub fn get(&self, what_to_get: HccParamsPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn is_set(&self, what_to_check: HccParamsBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }
}

pub struct HcspPortRoute {
    address: u64,
}

impl HcspPortRoute {
    #[inline(always)]
    pub fn new(address: u64) -> HcspPortRoute {
        return HcspPortRoute { address };
    }
}
#[repr(u32)]
pub enum FrameListSize {
    Count1024 = 0,
    Count512 = 1,
    Count256 = 2,
}

impl FrameListSize {
    #[inline(always)]
    pub fn new(value: u32) -> FrameListSize {
        return match value {
            0 => FrameListSize::Count1024,
            1 => FrameListSize::Count512,
            2 => FrameListSize::Count256,
            _ => simple_kernel_panic("FrameListSize/new", "Invalid value\n"),
        };
    }
}
#[repr(u32)]
pub enum InterruptThresholdControl {
    MicroFrames1 = 1,
    MicroFrames2 = 2,
    MicroFrames4 = 4,
    MicroFrames8 = 8,   // 1ms
    MicroFrames16 = 16, // 2ms
    MicroFrames32 = 32, // 4ms
    MicroFrames64 = 64, // 8ms
}

impl InterruptThresholdControl {
    #[inline(always)]
    pub fn new(value: u32) -> InterruptThresholdControl {
        return match value {
            1 => Self::MicroFrames1,
            2 => Self::MicroFrames2,
            4 => Self::MicroFrames4,
            8 => Self::MicroFrames8,
            16 => Self::MicroFrames16,
            32 => Self::MicroFrames32,
            64 => Self::MicroFrames64,
            _ => simple_kernel_panic("InterruptThresholdControl/new", "Invalid value\n"),
        };
    }
}

pub struct UsbCmd {
    val: *mut u32,
}

#[repr(u32)]
pub enum UsbCmdPart {
    FrameListSize = (0x3 << 16) | 2,
    AsynchronousScheduleParkModeCount = (0x3 << 16) | 8,
    InterruptThresholdControl = (0xFF << 16) | 16,
}
#[repr(u32)]
pub enum UsbCmdBitPart {
    /** Run/Stop */
    Rs = 0,
    /** Host Controller Reset*/
    HcReset = 1,
    PeriodicScheduleEnable = 4,
    AsynchronousScheduleEnable = 5,
    InterruptOnAsyncAdvanceDoorbell = 6,
    LightHostControllerReset = 7,
    AsynchronousScheduleParkModeEnable = 11,
}

impl UsbCmd {
    #[inline(always)]
    pub fn new(address: u64) -> UsbCmd {
        return UsbCmd {
            val: address as *mut u32,
        };
    }

    pub fn is_set(&self, what_to_check: UsbCmdBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: UsbCmdBitPart, val: bool) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe {
            self.val
                .write_volatile(prev_val | ((val as u32) << set_u32))
        }
    }

    pub fn get(&self, what_to_get: UsbCmdPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: UsbCmdPart, val: u32) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.val.write_volatile(prev_val) }
    }
}
#[repr(u32)]
pub enum UsbStsBitPart {
    /** USB Interrupt*/
    UsbInt = 0,
    /** USB Error Interrupt*/
    UsbErrInt = 1,
    PortChangeDetected = 2,
    /** Clears on Write*/
    FrameListRollOver = 3,
    HostSystemError = 4,
    InterruptOnAsyncAdvance = 5,
    HcHalted = 12,
    Reclamation = 13,
    PeriodicScheduleStatus = 14,
    AsynchronousScheduleStatus = 15,
}

pub struct UsbSts {
    val: *mut u32,
}
impl UsbSts {
    #[inline(always)]
    pub fn new(address: u64) -> UsbSts {
        return UsbSts {
            val: address as *mut u32,
        };
    }
    #[inline(always)]
    pub fn as_u32(&self) -> u32 {
        return unsafe { *self.val };
    }

    pub fn is_set(&self, what_to_check: UsbStsBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: UsbStsBitPart) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }
    /*
    pub fn get(&self, what_to_get: UsbStsPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: UsbStsPart, val: u32) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.val.write_volatile(prev_val) }
    }
    */
}

#[repr(u32)]
pub enum UsbIntrBitPart {
    UsbInterruptEnable = 0,
    UsbErrorInterruptEnable = 1,
    PortChangeInterruptEnable = 2,
    FrameListRolloverEnable = 3,
    HostSystemErrorEnable = 4,
    InterruptOnAsyncAdvanceEnable = 5,
}

pub struct UsbIntr {
    val: *mut u32,
}
impl UsbIntr {
    #[inline(always)]
    pub fn new(address: u64) -> UsbIntr {
        return UsbIntr {
            val: address as *mut u32,
        };
    }

    pub fn is_set(&self, what_to_check: UsbStsBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: UsbIntrBitPart) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }
}
#[derive(Clone)]
pub struct FrIndex {
    address: *mut u32,
}

pub const FRAME_INDEX_MASK: u32 = 0b11111111_111111;

impl FrIndex {
    pub const fn empty() -> FrIndex {
        return FrIndex {
            address: null_mut(),
        };
    }

    #[inline(always)]
    pub fn new(address: u64) -> FrIndex {
        return FrIndex {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn frame_index(&self) -> u16 {
        return (unsafe { self.address.read_volatile() & FRAME_INDEX_MASK } >> 3) as u16;
    }

    #[inline(always)]
    pub fn raw_frame_index(&self) -> u16 {
        return unsafe { self.address.read_volatile() & FRAME_INDEX_MASK } as u16;
    }

    #[inline(always)]
    pub fn set_frame_index(&mut self, frame_index: u16) {
        unsafe { self.address.write_volatile((frame_index << 3) as u32) };
    }
}

pub struct CtrldSSegment {
    address: *mut u32,
}
impl CtrldSSegment {
    #[inline(always)]
    pub fn new(address: u64) -> CtrldSSegment {
        return CtrldSSegment {
            address: address as *mut u32,
        };
    }
}

pub struct PeriodicListBase {
    address: *mut u32,
}
impl PeriodicListBase {
    #[inline(always)]
    pub fn new(address: u64) -> PeriodicListBase {
        return PeriodicListBase {
            address: address as *mut u32,
        };
    }
    #[inline(always)]
    pub fn get_base_address(&self) -> u64 {
        return unsafe { self.address.read_volatile() } as u64;
    }
    #[inline(always)]
    pub fn set_base_address(&mut self, address: u32) {
        return unsafe { self.address.write_volatile(address) };
    }
}

pub struct AsyncListAddr {
    address: *mut u32,
}
impl AsyncListAddr {
    #[inline(always)]
    pub fn new(address: u64) -> AsyncListAddr {
        return AsyncListAddr {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn set_address(&mut self, address: u32) {
        unsafe { self.address.write_volatile(address) };
    }
    #[inline(always)]
    pub fn get_address(&self) -> u32 {
        return unsafe { self.address.read_volatile() };
    }
}

pub struct ConfigFlag {
    address: *mut u32,
}
impl ConfigFlag {
    #[inline(always)]
    pub fn new(address: u64) -> ConfigFlag {
        return ConfigFlag {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn cf(&self) -> bool {
        return 1 == unsafe { self.address.read_volatile() & 1 };
    }
    #[inline(always)]
    pub fn set_cf(&mut self, cf: bool) {
        unsafe {
            self.address.write_volatile(cf as u32);
        }
    }
}

pub enum LineStatus {
    Se0,
    J,
    K,
}
impl LineStatus {
    #[inline(always)]
    pub fn new(value: u32) -> Self {
        return match value {
            0b00 => Self::Se0,
            0b10 => Self::J,
            0b01 => Self::K,
            _ => simple_kernel_panic("LineStatus/new", "Invalid value\n"),
        };
    }
}

pub enum PortIndicatorControl {
    IndicatorsOff,
    Amber,
    Green,
}
impl PortIndicatorControl {
    #[inline(always)]
    pub fn new(value: u32) -> Self {
        return match value {
            0b00 => Self::IndicatorsOff,
            0b01 => Self::Amber,
            0b10 => Self::Green,
            _ => simple_kernel_panic("PortIndicatorControl/new", "Invalid value\n"),
        };
    }
}

pub enum PortTestControl {
    TestModeNotEnabled,
    JStateTest,
    KStateTest,
    Se0NakTest,
    PacketTest,
    ForceEnableTest,
}
impl PortTestControl {
    #[inline(always)]
    pub fn new(value: u32) -> Self {
        return match value {
            0b0000 => Self::TestModeNotEnabled,
            0b0001 => Self::JStateTest,
            0b0010 => Self::KStateTest,
            0b0011 => Self::Se0NakTest,
            0b0100 => Self::PacketTest,
            0b0101 => Self::ForceEnableTest,
            _ => simple_kernel_panic("PortTestControl/new", "Invalid value\n"),
        };
    }
    #[inline(always)]
    pub fn as_u32(&self) -> u32 {
        return match self {
            Self::TestModeNotEnabled => 0b0000,
            Self::JStateTest => 0b0001,
            Self::KStateTest => 0b0010,
            Self::Se0NakTest => 0b0011,
            Self::PacketTest => 0b0100,
            Self::ForceEnableTest => 0b0101,
        };
    }
}

#[repr(u32)]
pub enum PortScPart {
    LineStatus = (0x3 << 16) | 10,
    PortIndicatorControl = (0x3 << 16) | 14,
    PortTestControl = (0xF << 16) | 16,
}
#[repr(u32)]
pub enum PortScBitPart {
    CurrentConnectStatus = 0,
    ConnectStatusChange = 1,
    PortEnabledDisabled = 2,
    PortEnabledDisabledChanged = 3,
    OverCurrentActive = 4,
    OverCurrentChange = 5,
    ForcePortResume = 6,
    Suspend = 7,
    PortReset = 8,
    PortPower = 12,
    PortOwner = 13,
    WakeOnConnectEnable = 20,
    WakeOnDisconnectEnable = 21,
    WakeOnOverCurrentEnable = 22,
}

pub struct PortSc {
    val: *mut u32,
}
impl PortSc {
    #[inline(always)]
    pub fn new(address: u64) -> PortSc {
        return PortSc {
            val: address as *mut u32,
        };
    }

    pub fn is_set(&self, what_to_check: PortScBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: PortScBitPart, val: bool) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe {
            self.val
                .write_volatile(prev_val | ((val as u32) << set_u32))
        }
    }

    pub fn get(&self, what_to_get: PortScPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: PortScPart, val: u32) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.val.write_volatile(prev_val) }
    }
}

pub struct UsbBase {
    physical_address: u64,
    virtual_address: u64,
    cap_length: CapLength,
}

type CapLength = u8;
type HciVersion = u16;
impl UsbBase {
    pub const fn empty() -> UsbBase {
        return Self {
            physical_address: 0,
            virtual_address: 0,
            cap_length: 0,
        };
    }

    #[inline(always)]
    pub fn new(physical_address: u64, virtual_address: u64) -> UsbBase {
        let cap_length = unsafe { (virtual_address as *const CapLength).read_volatile() };
        return UsbBase {
            physical_address,
            virtual_address,
            cap_length,
        };
    }

    #[inline(always)]
    pub fn caplength(&self) -> CapLength {
        return self.cap_length;
    }

    #[inline(always)]
    pub fn hciversion(&self) -> HciVersion {
        return unsafe { ((self.virtual_address + 2) as *const HciVersion).read_volatile() };
    }

    #[inline(always)]
    pub fn hcsparams(&self) -> HcsParams {
        return HcsParams::new(self.virtual_address + 4);
    }

    #[inline(always)]
    pub fn hccparams(&self) -> HccParams {
        return HccParams::new(self.virtual_address + 8);
    }

    #[inline(always)]
    pub fn hcsp_portroute(&self) -> HcspPortRoute {
        return HcspPortRoute::new(self.virtual_address + 0xC);
    }

    #[inline(always)]
    pub fn usbcmd(&self) -> UsbCmd {
        return UsbCmd::new(self.virtual_address + self.cap_length as u64);
    }
    #[inline(always)]
    pub fn usbsts(&self) -> UsbSts {
        return UsbSts::new(self.virtual_address + 4 + self.cap_length as u64);
    }
    #[inline(always)]
    pub fn usbintr(&self) -> UsbIntr {
        return UsbIntr::new(self.virtual_address + 8 + self.cap_length as u64);
    }
    #[inline(always)]
    pub fn frindex(&self) -> FrIndex {
        return FrIndex::new(self.virtual_address + 0xC + self.cap_length as u64);
    }
    #[inline(always)]
    pub fn ctrldssegment(&self) -> CtrldSSegment {
        return CtrldSSegment::new(self.virtual_address + 0x10 + self.cap_length as u64);
    }
    #[inline(always)]
    pub fn periodiclistbase(&self) -> PeriodicListBase {
        return PeriodicListBase::new(self.virtual_address + 0x14 + self.cap_length as u64);
    }
    #[inline(always)]
    pub fn asynclistaddr(&self) -> AsyncListAddr {
        return AsyncListAddr::new(self.virtual_address + 0x18 + self.cap_length as u64);
    }
    #[inline(always)]
    pub fn configflag(&self) -> ConfigFlag {
        return ConfigFlag::new(self.virtual_address + 0x40 + self.cap_length as u64);
    }
    #[inline(always)]
    pub fn portsc(&self, index: u8) -> PortSc {
        return PortSc::new(
            self.virtual_address + 0x44 + self.cap_length as u64 + (index as u64) * 4,
        );
    }
}

pub struct Fladj {
    address: u64,
}

impl Fladj {
    #[inline(always)]
    pub const fn new(address: u64) -> Fladj {
        return Fladj { address };
    }

    #[inline(always)]
    pub fn set(&mut self, value: u8) {
        unsafe { (self.address as *mut u8).write_volatile(value) };
    }
}
