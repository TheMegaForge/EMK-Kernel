use core::{
    cell::UnsafeCell,
    ffi::c_void,
    ptr::{null, null_mut, read_volatile},
};

use crate::{
    drivers::usb::ohci::{
        data_structures::HostControllerFunctionalState::{
            UsbOperational, UsbReset, UsbResume, UsbSuspend,
        },
        structures::endpoint::{EndpointDescriptorBitPart::S, OhciEndpointDescriptor},
    },
    hal::print::simple_kernel_panic,
};

#[repr(u32)]
pub enum OhciControlBitPart {
    /** PeriodicListEnable */
    Ple = 2,
    /** IsochronousEnable */
    Ie = 3,
    /** ControlListEnable */
    Cle = 4,
    /** BulkListEnable */
    Ble = 5,
    /** InterruptRouting*/
    Ir = 8,
    /** RemoteWakeupConnected*/
    Rwc = 9,
    /** RemoteWakeupEnable */
    Rwe = 10,
}
#[repr(u8)]
pub enum HostControllerFunctionalState {
    UsbReset = 0,
    UsbResume = 1,
    UsbOperational = 2,
    UsbSuspend = 3,
}

pub fn hcfs_into_enum(hcfs: u8) -> HostControllerFunctionalState {
    return match hcfs {
        0 => UsbReset,
        1 => UsbResume,
        2 => UsbOperational,
        3 => UsbSuspend,
        _ => panic!("Invalid hcfs value\n"),
    };
}

#[repr(u32)]
pub enum OhciControlPart {
    /** ControlBulkServiceRatio */
    Cbsr = (0x03 << 16) | 0,
    /** HostControllerFunctionalState */
    Hcfs = (0x03 << 16) | 6,
}

pub struct OhciControl {
    val: *mut u32,
}

impl OhciControl {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn disable_all_processing(&mut self) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        prev_val &= !(0xF << 2);
        unsafe { self.val.write_volatile(prev_val) }
    }

    pub fn enable_all_processing(&mut self) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        prev_val |= !(0xF << 2);
        unsafe { self.val.write_volatile(prev_val) }
    }

    pub fn set_all_queues_on(&mut self) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        prev_val |= 0xF << 2; // Ple, Ie, Cle, Ble
        unsafe { self.val.write_volatile(prev_val) };
    }

    pub fn is_set(&self, what_to_check: OhciControlBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: OhciControlBitPart, val: bool) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe {
            self.val
                .write_volatile(prev_val | ((val as u32) << set_u32))
        }
    }

    pub fn get(&self, what_to_get: OhciControlPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: OhciControlPart, val: u32) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.val.write_volatile(prev_val) }
    }
}
#[repr(u32)]
pub enum OhciCommandStatusBitPart {
    /** HostControllerReset */
    Hcr = 0,
    /** ControlListFilled */
    Clf = 1,
    /** BulkListFilled */
    Blf = 2,
    /** OwnershipChangeRequest */
    Ocr = 3,
}

#[repr(u32)]
pub enum OhciCommandStatusPart {
    /** SchedulingOverrunCount */
    Soc = (0x03 << 16) | 16,
}

pub struct OhciCommandStatus {
    val: *mut u32,
}

impl OhciCommandStatus {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn is_set(&self, what_to_check: OhciCommandStatusBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: OhciCommandStatusBitPart) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }

    pub fn get(&self, what_to_get: OhciCommandStatusPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: OhciCommandStatusPart, val: u32) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.val.write_volatile(prev_val) }
    }
}
#[repr(u32)]
pub enum OhciInterrupt {
    /** SchedulingOverrun*/
    So = 0,
    /** WritebackDoneHead */
    Wdh = 1,
    /** StartofFrame */
    Sf = 2,
    /** ResumeDetected*/
    Rd = 3,
    /** UnrecoverableError */
    Ue = 4,
    /** FrameNumberOverflow */
    Fno = 5,
    /** RootHubStatusChange */
    Rhsc = 6,
    /** OwnershipChange */
    Oc = 30,
    /** Master Interrupt Enable*/
    Mie = 31,
}

pub struct OhciInterruptStatus {
    val: *mut u32,
}

impl OhciInterruptStatus {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn as_u32(&self) -> u32 {
        return unsafe { self.val.read_volatile() };
    }

    pub fn is_set(&self, what_to_check: OhciInterrupt) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: OhciInterrupt) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }
}

pub struct OhciInterruptEnable {
    val: *mut u32,
}

impl OhciInterruptEnable {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn enable_all_except_sf(&mut self) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        prev_val &= !(1 << 2);
        prev_val |= 1 << 30 | 0xF << 3 | 0x3;
        unsafe { self.val.write_volatile(prev_val) }
    }

    pub fn enable(&mut self, what_to_set: OhciInterrupt) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }
}

pub struct OhciInterruptDisable {
    val: *mut u32,
}

impl OhciInterruptDisable {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn disable(&mut self, what_to_set: OhciInterrupt) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }
}
#[repr(u32)]
pub enum OhciFmIntervalBitPart {
    /** FrameIntervalToggle*/
    Fit = 31,
}

#[repr(u32)]
pub enum OhciFmIntervalPart {
    /** FrameInterval */
    Fi = (0x3FFF << 16) | 0,
    /** FSLargestDataPacket */
    Fsmps = (0x7FFF << 16) | 16,
}

pub struct OhciFmInterval {
    val: *mut u32,
}

impl OhciFmInterval {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn as_u32(&self) -> u32 {
        return unsafe { self.val.read_volatile() };
    }

    pub fn is_set(&self, what_to_check: OhciFmIntervalBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: OhciFmIntervalBitPart) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }

    pub fn get(&self, what_to_get: OhciFmIntervalPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: OhciFmIntervalPart, val: u32) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.val.write_volatile(prev_val) }
    }
}

#[repr(u32)]
pub enum OhciFmRemainingBitPart {
    /** FrameRemainingToggle */
    Frt = 31,
}

#[repr(u32)]
pub enum OhciFmRemainingPart {
    /** FrameRemaining*/
    Fr = (0x3FFF << 16) | 0,
}

pub struct OhciFmRemaining {
    val: *mut u32,
}

impl OhciFmRemaining {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn is_set(&self, what_to_check: OhciFmRemainingBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: OhciFmRemainingBitPart) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }

    pub fn get(&self, what_to_get: OhciFmRemainingPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: OhciFmRemainingPart, val: u32) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.val.write_volatile(prev_val) }
    }
}

#[repr(u32)]
pub enum OhciRhDescriptorABitPart {
    /** PowerSwitchingMode */
    Psm = 8,
    /** NoPowerSwitching */
    Nps = 9,
    /** DeviceType */
    Dt = 10,
    /** OverCurrentProtectionMode */
    Ocpm = 11,
    /** NoOverCurrentProtection */
    Nocp = 12,
}

#[repr(u32)]
pub enum OhciRhDescriptorAPart {
    /** NumberDownstreamPorts*/
    Ndp = (0xFF << 16) | 0,
    /** PowerOnToPowerGoodTime*/
    Potpgt = (0xFF << 16) | 24,
}

pub struct OhciRhDescriptorA {
    val: *mut u32,
}

impl OhciRhDescriptorA {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn is_set(&self, what_to_check: OhciRhDescriptorABitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: OhciRhDescriptorABitPart) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }

    pub fn get(&self, what_to_get: OhciRhDescriptorAPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set_part(&self, what_to_get: OhciRhDescriptorAPart, val: u32) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_get as u32;
        prev_val &= !((set_u32 >> 16) << (set_u32 & 0xFF));
        prev_val |= (val & (set_u32 >> 16)) << (set_u32 & 0xFF);
        unsafe { self.val.write_volatile(prev_val) }
    }
}
#[repr(u32)]
pub enum OhciRhDescriptorBPart {
    /** DeviceRemovable*/
    Dr = (0xFFFF << 16) | 0,
    /** PortPowerControlMask */
    Ppcm = (0xFFFF << 16) | 16,
}

pub struct OhciRhDescriptorB {
    val: *mut u32,
}

impl OhciRhDescriptorB {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn get(&self, what_to_get: OhciRhDescriptorBPart) -> u32 {
        let val = unsafe { self.val.read_volatile() };
        let get_u32 = what_to_get as u32;
        return (val >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }

    pub fn set(&self, what_to_get: OhciRhDescriptorBPart, val: u32) {
        let get_u32 = what_to_get as u32;
        let mut prev_val = unsafe { self.val.read_volatile() };
        prev_val &= !((get_u32 >> 16) << (get_u32 & 0xFF));
        prev_val |= (val & (get_u32 >> 16)) << (get_u32 & 0xFF);
        unsafe { self.val.write_volatile(prev_val) }
    }
}
#[repr(u32)]
pub enum OhciRhStatusBitPart {
    /** LocalPowerStatus */
    Lps = 0,
    /** OverCurrentIndicator */
    Oci = 1,
    /** DeviceRemoteWakeupEnable */
    Drwe = 15,
    /** LocalPowerStatusChange */
    Lpsc = 16,
    /** OverCurrentIndicatorChange */
    Ocic = 17,
    /** ClearRemoteWakeupEnable */
    Crwe = 31,
}

pub struct OhciRhStatus {
    val: *mut u32,
}

impl OhciRhStatus {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn is_set(&self, what_to_check: OhciRhStatusBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: OhciRhStatusBitPart) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }
}

pub enum OhciRhPortStatusBitPart {
    /** CurrentConnectStatus*/
    Ccs = 0,
    /** PortEnableStatus*/
    Pes = 1,
    /** PortSuspendStatus*/
    Pss = 2,
    /** PortOverCurrentIndicator*/
    Poci = 3,
    /** PortResetStatus */
    Prs = 4,
    /** PortPowerStatus */
    Pps = 8,
    /** LowSpeedDeviceAttached */
    Lsda = 9,
    /** ConnectStatusChange */
    Csc = 16,
    /** PortEnableStatusChange */
    Pesc = 17,
    /** PortSuspendStatusChange */
    Pssc = 18,
    /** PortOverCurrentIndicatorChange */
    Ocic = 19,
    /** PortResetStatusChange */
    Prsc = 20,
}

pub struct OhciRhPortStatus {
    val: *mut u32,
}

impl OhciRhPortStatus {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn is_set(&self, what_to_check: OhciRhPortStatusBitPart) -> bool {
        let val = unsafe { self.val.read_volatile() };
        return 0 != (val & (1 << what_to_check as u32));
    }

    pub fn set(&mut self, what_to_set: OhciRhPortStatusBitPart) {
        let mut prev_val = unsafe { self.val.read_volatile() };
        let set_u32 = what_to_set as u32;
        prev_val &= !(1 << set_u32);
        unsafe { self.val.write_volatile(prev_val | (1 << set_u32)) }
    }
}

pub struct OhciBar {
    bar: *mut u32,
}

impl OhciBar {
    pub const fn empty() -> Self {
        return Self { bar: null_mut() };
    }

    pub fn address(&self) -> *mut u32 {
        return self.bar;
    }

    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            bar: addr as *mut u32,
        };
    }

    pub fn hc_revision(&self) -> u8 {
        return (unsafe { self.bar.read_volatile() } & 0xFF) as u8;
    }
    pub fn hc_control(&self) -> OhciControl {
        OhciControl::new(unsafe { self.bar.add(1) })
    }
    pub fn hc_command_status(&self) -> OhciCommandStatus {
        OhciCommandStatus::new(unsafe { self.bar.add(2) })
    }
    pub fn hc_interrupt_status(&self) -> OhciInterruptStatus {
        OhciInterruptStatus::new(unsafe { self.bar.add(3) })
    }
    pub fn hc_interrupt_enable(&self) -> OhciInterruptEnable {
        OhciInterruptEnable::new(unsafe { self.bar.add(4) })
    }
    pub fn hc_interrupt_disable(&self) -> OhciInterruptDisable {
        OhciInterruptDisable::new(unsafe { self.bar.add(5) })
    }

    pub fn hc_control_head_ed(&mut self) -> u32 {
        unsafe { self.bar.add(8).read_volatile() }
    }

    pub fn hc_bulk_head_ed(&mut self) -> u32 {
        unsafe { self.bar.add(10).read_volatile() }
    }

    pub fn hc_hcca(&self) -> u32 {
        unsafe { self.bar.add(6).read_volatile() }
    }
    pub fn write_hc_hcca(&mut self, val: u32) {
        unsafe { self.bar.add(6).write_volatile(val) }
    }
    pub fn write_hc_period_current_ed(&mut self, val: u32) {
        unsafe { self.bar.add(7).write_volatile(val) }
    }
    pub fn write_hc_control_head_ed(&mut self, val: u32) {
        unsafe { self.bar.add(8).write_volatile(val) }
    }
    pub fn write_hc_control_current_ed(&mut self, val: u32) {
        unsafe { self.bar.add(9).write_volatile(val) }
    }

    pub fn write_hc_bulk_head_ed(&mut self, val: u32) {
        unsafe { self.bar.add(10).write_volatile(val) }
    }

    pub fn write_hc_bulk_current_ed(&mut self, val: u32) {
        unsafe { self.bar.add(11).write_volatile(val) }
    }
    pub fn write_hc_done_head(&mut self, val: u32) {
        unsafe { self.bar.add(12).write_volatile(val) }
    }

    pub fn hc_done_head(&mut self) -> u32 {
        unsafe { self.bar.add(12).read_volatile() }
    }
    pub fn hc_fm_interval(&self) -> OhciFmInterval {
        OhciFmInterval::new(unsafe { self.bar.add(13) })
    }
    pub fn hc_fm_remaining(&self) -> OhciFmRemaining {
        OhciFmRemaining::new(unsafe { self.bar.add(14) })
    }
    pub fn hc_fm_number(&self) -> u16 {
        return unsafe { self.bar.add(15).read_volatile() & 0xFFFF } as u16;
    }
    pub fn write_hc_periodic_start(&mut self, val: u16) {
        unsafe { self.bar.add(16).write_volatile(val as u32) };
    }
    pub fn write_hc_ls_threshold(&mut self, val: u16) {
        unsafe { self.bar.add(17).write_volatile(val as u32) };
    }
    pub fn hc_rh_descriptor_a(&self) -> OhciRhDescriptorA {
        return OhciRhDescriptorA::new(unsafe { self.bar.add(18) });
    }
    pub fn hc_rh_descriptor_b(&self) -> OhciRhDescriptorB {
        return OhciRhDescriptorB::new(unsafe { self.bar.add(19) });
    }
    pub fn hc_rh_status(&self) -> OhciRhStatus {
        return OhciRhStatus::new(unsafe { self.bar.add(20) });
    }
    /** Notice: Port 1 is outlined in the Ohci Specification */
    pub fn hc_rh_port_status(&self, port: u32) -> OhciRhPortStatus {
        OhciRhPortStatus::new(unsafe { self.bar.add(21 + (port as usize - 1)) })
    }
}

pub struct OhciHcca {
    val: *mut c_void,
}

impl OhciHcca {
    pub const fn new(val: *mut c_void) -> Self {
        return Self { val };
    }
    pub fn interrupt_list(&self, index: u8) -> u32 {
        assert!(31 >= index);
        unsafe { (self.val as *const u32).add(index as usize).read_volatile() }
    }

    pub fn write_interrupt_list(&mut self, index: u8, val: u32) {
        assert!(31 >= index);
        unsafe {
            (self.val as *mut u32)
                .add(index as usize)
                .write_volatile(val)
        }
    }

    pub fn frame_number(&self) -> u16 {
        unsafe { (self.val.add(0x80) as *const u16).read_volatile() }
    }
    pub fn pad1(&self) -> u16 {
        unsafe { (self.val.add(0x82) as *const u16).read_volatile() }
    }

    pub fn done_head(&self) -> u32 {
        unsafe { (self.val.add(0x84) as *const u32).read_volatile() }
    }
}
