use core::ptr::null_mut;

use crate::{drivers::usb::independent::HciState, hal::print::simple_kernel_panic};

pub struct HcsParams {
    address: *const u32,
}

type PortRoutingRule = u8;

pub const N_PORTS_MASK: u32 = 0b1111;
pub const N_PCC_MASK: u32 = 0b1111;
pub const N_CC_MASK: u32 = 0b1111;
pub const DEBUG_PORT_NUMBER_MASK: u32 = 0b1111;

impl HcsParams {
    #[inline(always)]
    pub fn new(address: u64) -> HcsParams {
        return HcsParams {
            address: address as *const u32,
        };
    }

    /*
     *
     */
    #[inline(always)]
    pub fn n_ports(&self) -> u8 {
        return unsafe { self.address.read_volatile() & N_PORTS_MASK } as u8;
    }

    #[inline(always)]
    pub fn ppc(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 4) & 1 };
    }

    #[inline(always)]
    pub fn port_routing_rules(&self) -> PortRoutingRule {
        return unsafe { (self.address.read_volatile() >> 7) & 1 } as PortRoutingRule;
    }

    #[inline(always)]
    pub fn n_pcc(&self) -> u8 {
        return unsafe { (self.address.read_volatile() >> 8) & N_PCC_MASK } as u8;
    }

    #[inline(always)]
    pub fn n_cc(&self) -> u8 {
        return unsafe { (self.address.read_volatile() >> 12) & N_CC_MASK } as u8;
    }

    #[inline(always)]
    pub fn p_indicator(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 16) & 1 };
    }

    #[inline(always)]
    pub fn debug_port_number(&self) -> u8 {
        return unsafe { (self.address.read_volatile() >> 20) & DEBUG_PORT_NUMBER_MASK } as u8;
    }
}

pub struct HccParams {
    address: *const u32,
}

pub const ISOCHRONOUS_SCHEDULING_THRESHOLD: u32 = 0b1111;
pub const EECP_MASK: u32 = 0b11111111;

impl HccParams {
    #[inline(always)]
    pub fn new(address: u64) -> HccParams {
        return HccParams {
            address: address as *const u32,
        };
    }

    #[inline(always)]
    pub fn qword_addressing_capability(&self) -> u8 {
        return unsafe { self.address.read_volatile() & 1 } as u8;
    }

    #[inline(always)]
    pub fn programmable_frame_list_flag(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 1) & 1 };
    }
    #[inline(always)]
    pub fn asynchronous_schedule_park_capability(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 2) & 1 };
    }
    #[inline(always)]
    pub fn isochronous_scheduling_threshold(&self) -> u8 {
        return unsafe { (self.address.read_volatile() >> 4) & ISOCHRONOUS_SCHEDULING_THRESHOLD }
            as u8;
    }
    #[inline(always)]
    pub fn eecp(&self) -> u8 {
        return unsafe { (self.address.read_volatile() >> 8) & EECP_MASK } as u8;
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

pub enum FrameListSize {
    Count1024,
    Count512,
    Count256,
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

    #[inline(always)]
    pub fn as_u32(&self) -> u32 {
        return match self {
            Self::Count1024 => 0,
            Self::Count512 => 1,
            Self::Count256 => 2,
        };
    }
}

pub enum InterruptThresholdControl {
    MicroFrames1,
    MicroFrames2,
    MicroFrames4,
    MicroFrames8,  // 1ms
    MicroFrames16, // 2ms
    MicroFrames32, // 4ms
    MicroFrames64, // 8ms
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

    #[inline(always)]
    pub fn as_u32(&self) -> u32 {
        return match self {
            Self::MicroFrames1 => 1,
            Self::MicroFrames2 => 2,
            Self::MicroFrames4 => 4,
            Self::MicroFrames8 => 8,
            Self::MicroFrames16 => 16,
            Self::MicroFrames32 => 32,
            Self::MicroFrames64 => 64,
        };
    }
}

pub struct UsbCmd {
    address: *mut u32,
}

pub const SET_RS_MASK: u32 = 1 << 0;
pub const SET_HCRESET_MASK: u32 = 1 << 1;
pub const SET_FRAME_LIST_SIZE_MASK: u32 = 0b11 << 2;
pub const SET_PERIODIC_SCHEDULE_ENABLE_MASK: u32 = 1 << 4;
pub const SET_ASYNCHRONOUS_SCHEDULE_ENABLE_MASK: u32 = 1 << 5;
pub const SET_INTERRUPT_ON_ASYNC_ADVANCE_DOORBELL_MASK: u32 = 1 << 6;
pub const SET_INTERRUPT_THRESHOLD_CONTROL_MASK: u32 = 0b11111111 << 16;

pub const INTERRUPT_THRESHOLD_CONTROL_MASK: u32 = 0b11111111;

impl UsbCmd {
    #[inline(always)]
    pub fn new(address: u64) -> UsbCmd {
        return UsbCmd {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn rs(&self) -> HciState {
        return HciState::from_bool(1 == unsafe { self.address.read_volatile() & 1 });
    }

    #[inline(always)]
    pub fn hcreset(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 1) & 1 };
    }
    #[inline(always)]
    pub fn frame_list_size(&self) -> FrameListSize {
        return FrameListSize::new((unsafe { self.address.read_volatile() } >> 2) & 2);
    }
    #[inline(always)]
    pub fn periodic_schedule_enable(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 4) & 1 };
    }
    #[inline(always)]
    pub fn asynchronous_schedule_enable(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 5) & 1 };
    }
    #[inline(always)]
    pub fn interrupt_on_async_advance_doorbell(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 6) & 1 };
    }
    #[inline(always)]
    pub fn interrupt_threshold_control(&self) -> InterruptThresholdControl {
        return InterruptThresholdControl::new(
            (unsafe { self.address.read_volatile() } >> 16) & INTERRUPT_THRESHOLD_CONTROL_MASK,
        );
    }

    #[inline(always)]
    pub fn set_rs(&mut self, val: bool) {
        unsafe {
            self.address
                .write_volatile((self.address.read_volatile() & (!SET_RS_MASK)) | val as u32);
        }
    }
    #[inline(always)]
    pub fn set_hcreset(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                (self.address.read_volatile() & (!SET_HCRESET_MASK)) | (val as u32) << 1,
            );
        }
    }
    #[inline(always)]
    pub fn set_frame_list_size(&mut self, frame_list_size: FrameListSize) {
        unsafe {
            self.address.write_volatile(
                (self.address.read_volatile() & (!SET_FRAME_LIST_SIZE_MASK))
                    | (frame_list_size.as_u32() << 2),
            );
        }
    }
    #[inline(always)]
    pub fn set_periodic_schedule_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                (self.address.read_volatile() & (!SET_PERIODIC_SCHEDULE_ENABLE_MASK))
                    | ((val as u32) << 4),
            );
        }
    }
    #[inline(always)]
    pub fn set_asynchronous_schedule_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                (self.address.read_volatile() & (!SET_ASYNCHRONOUS_SCHEDULE_ENABLE_MASK))
                    | ((val as u32) << 5),
            );
        }
    }
    #[inline(always)]
    pub fn set_interrupt_on_async_advance_doorbell(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                (self.address.read_volatile() & (!SET_INTERRUPT_ON_ASYNC_ADVANCE_DOORBELL_MASK))
                    | ((val as u32) << 6),
            );
        }
    }

    pub fn set_interrupt_threshold_control(&mut self, threshold: InterruptThresholdControl) {
        unsafe {
            let value = self.address.read_volatile() & (!SET_INTERRUPT_THRESHOLD_CONTROL_MASK);
            self.address
                .write_volatile(value | (threshold.as_u32() << 16));
        }
    }
}

pub struct UsbSts {
    address: *mut u32,
}
impl UsbSts {
    #[inline(always)]
    pub fn new(address: u64) -> UsbSts {
        return UsbSts {
            address: address as *mut u32,
        };
    }
    #[inline(always)]
    pub fn as_u32(&self) -> u32 {
        return unsafe { *self.address };
    }

    #[inline(always)]
    pub fn usbint(&self) -> bool {
        return 1 == unsafe { self.address.read_volatile() & 1 };
    }
    #[inline(always)]
    pub fn usberrint(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 1) & 1 };
    }
    #[inline(always)]
    pub fn port_change_detected(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 2) & 1 };
    }
    #[inline(always)]
    pub fn frame_list_rollover(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 3) & 1 };
    }
    #[inline(always)]
    pub fn host_system_error(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 4) & 1 };
    }
    #[inline(always)]
    pub fn interrupt_on_async_advance(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 5) & 1 };
    }
    #[inline(always)]
    pub fn hchalted(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 12) & 1 };
    }
    #[inline(always)]
    pub fn reclamation(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 13) & 1 };
    }
    #[inline(always)]
    pub fn periodic_schedule_status(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 14) & 1 };
    }
    #[inline(always)]
    pub fn asynchronous_schedule_status(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 15) & 1 };
    }
    #[inline(always)]

    pub fn clear_usbint(&mut self) {
        unsafe { self.address.write_volatile(1 << 0) };
    }
    #[inline(always)]
    pub fn clear_usberrint(&mut self) {
        unsafe { self.address.write_volatile(1 << 1) };
    }
    #[inline(always)]
    pub fn clear_port_change_detected(&mut self) {
        unsafe { self.address.write_volatile(1 << 2) };
    }
    #[inline(always)]
    pub fn clear_frame_list_rollover(&mut self) {
        unsafe { self.address.write_volatile(1 << 3) };
    }
    #[inline(always)]
    pub fn clear_host_system_error(&mut self) {
        unsafe { self.address.write_volatile(1 << 4) };
    }
    #[inline(always)]
    pub fn clear_interrupt_on_async_advance(&mut self) {
        unsafe { self.address.write_volatile(1 << 5) };
    }
}

pub const SET_USB_INTERRUPT_ENABLE_MASK: u32 = 1 << 0;
pub const SET_USB_ERROR_INTERRUPT_ENABLE_MASK: u32 = 1 << 1;
pub const SET_PORT_CHANGE_INTERRUPT_ENABLE_MASK: u32 = 1 << 2;
pub const SET_FRAME_LIST_ROLLOVER_ENABLE_MASK: u32 = 1 << 3;
pub const SET_HOST_SYSTEM_ERROR_ENABLE_MASK: u32 = 1 << 4;
pub const SET_INTERRUPT_ON_ASYNC_ADVANCE_ENABLE_MASK: u32 = 1 << 5;

pub struct UsbIntr {
    address: *mut u32,
}
impl UsbIntr {
    #[inline(always)]
    pub fn new(address: u64) -> UsbIntr {
        return UsbIntr {
            address: address as *mut u32,
        };
    }

    #[inline(always)]
    pub fn usb_interrupt_enable(&self) -> bool {
        return 1 == unsafe { self.address.read_volatile() & 1 };
    }
    #[inline(always)]
    pub fn usb_error_interrupt_enable(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 1) & 1 };
    }
    #[inline(always)]
    pub fn port_change_interrupt_enable(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 2) & 1 };
    }
    #[inline(always)]
    pub fn frame_list_rollover_enable(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 3) & 1 };
    }
    #[inline(always)]
    pub fn host_system_error_enable(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 4) & 1 };
    }
    #[inline(always)]
    pub fn interrupt_on_async_advance_enable(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 5) & 1 };
    }
    #[inline(always)]
    pub fn set_usb_interrupt_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_USB_INTERRUPT_ENABLE_MASK) | val as u32,
            );
        }
    }
    #[inline(always)]
    pub fn set_usb_error_interrupt_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_USB_ERROR_INTERRUPT_ENABLE_MASK)
                    | ((val as u32) << 1),
            );
        }
    }
    #[inline(always)]
    pub fn set_port_change_interrupt_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_PORT_CHANGE_INTERRUPT_ENABLE_MASK)
                    | ((val as u32) << 2),
            );
        }
    }
    #[inline(always)]
    pub fn set_frame_list_rollover_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_FRAME_LIST_ROLLOVER_ENABLE_MASK)
                    | ((val as u32) << 3),
            );
        }
    }
    #[inline(always)]
    pub fn set_host_system_error_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_HOST_SYSTEM_ERROR_ENABLE_MASK)
                    | ((val as u32) << 4),
            );
        }
    }
    #[inline(always)]
    pub fn set_interrupt_on_async_advance_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_INTERRUPT_ON_ASYNC_ADVANCE_ENABLE_MASK)
                    | ((val as u32) << 5),
            );
        }
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
    // Info: The value is being left shifted by 3
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

pub const LINE_STATUS_MASK: u32 = 0b11;
pub const PORT_INDICATOR_CONTROL_MASK: u32 = 0b11;
pub const PORT_TEST_CONTROL_MASK: u32 = 0b1111;

pub const SET_PORT_ENABLED_DISABLED_MASK: u32 = 1 << 2;
pub const SET_FORCE_PORT_RESUME_MASK: u32 = 1 << 6;
pub const SET_SUSPEND_MASK: u32 = 1 << 7;
pub const SET_PORT_RESET_MASK: u32 = 1 << 8;
pub const SET_PORT_POWER_MASK: u32 = 1 << 12;
pub const SET_PORT_OWNER_MASK: u32 = 1 << 13;
pub const SET_PORT_TEST_CONTROL_MASK: u32 = 0b1111 << 16;
pub const SET_WAKE_ON_CONNECT_ENABLE_MASK: u32 = 1 << 20;
pub const SET_WAKE_ON_DISCONNECT_ENABLE_MASK: u32 = 1 << 21;
pub const SET_WAKE_ON_OVER_CURRENT_ENABLE_MASK: u32 = 1 << 22;

pub struct PortSc {
    address: *mut u32,
}
impl PortSc {
    #[inline(always)]
    pub fn new(address: u64) -> PortSc {
        return PortSc {
            address: address as *mut u32,
        };
    }
    #[inline(always)]
    pub fn as_u32(&self) -> u32 {
        return unsafe { self.address.read_volatile() };
    }

    #[inline(always)]
    pub fn current_connect_status(&self) -> bool {
        return 1 == unsafe { self.address.read_volatile() & 1 };
    }
    #[inline(always)]
    pub fn connect_status_change(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 1) & 1 };
    }
    #[inline(always)]
    pub fn port_enabled_disabled(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 2) & 1 };
    }
    #[inline(always)]
    pub fn port_enable_disable_change(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 3) & 1 };
    }
    #[inline(always)]
    pub fn over_current_active(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 4) & 1 };
    }
    #[inline(always)]
    pub fn over_current_change(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 5) & 1 };
    }
    #[inline(always)]
    pub fn force_port_resume(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 6) & 1 };
    }
    #[inline(always)]
    pub fn suspend(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 7) & 1 };
    }
    #[inline(always)]
    pub fn port_reset(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 8) & 1 };
    }
    #[inline(always)]
    pub fn line_status(&self) -> LineStatus {
        return LineStatus::new(unsafe { self.address.read_volatile() >> 10 } & LINE_STATUS_MASK);
    }
    #[inline(always)]
    pub fn port_power(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 12) & 1 };
    }
    #[inline(always)]
    pub fn port_owner(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 13) & 1 };
    }
    #[inline(always)]
    pub fn port_indicator_control(&self) -> PortIndicatorControl {
        return PortIndicatorControl::new(
            unsafe { self.address.read_volatile() >> 15 } & PORT_INDICATOR_CONTROL_MASK,
        );
    }
    #[inline(always)]
    pub fn port_test_control(&self) -> PortTestControl {
        return PortTestControl::new(
            unsafe { self.address.read_volatile() >> 16 } & PORT_TEST_CONTROL_MASK,
        );
    }
    #[inline(always)]
    pub fn wake_on_connect_enable(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 20) & 1 };
    }
    #[inline(always)]
    pub fn wake_on_disconnect_enable(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 21) & 1 };
    }
    #[inline(always)]
    pub fn wake_on_over_current_enable(&self) -> bool {
        return 1 == unsafe { (self.address.read_volatile() >> 22) & 1 };
    }
    #[inline(always)]

    pub fn clear_connect_status_change(&mut self) {
        unsafe { self.address.write_volatile(1 << 1) };
    }
    #[inline(always)]
    pub fn set_port_enabled_disabled(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_PORT_ENABLED_DISABLED_MASK)
                    | (val as u32) << 2,
            )
        };
    }
    #[inline(always)]
    pub fn clear_port_enable_disable_change(&mut self) {
        unsafe { self.address.write_volatile(1 << 3) };
    }
    #[inline(always)]
    pub fn clear_over_current_change(&mut self) {
        unsafe { self.address.write_volatile(1 << 5) };
    }
    #[inline(always)]
    pub fn set_force_port_resume(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_FORCE_PORT_RESUME_MASK) | (val as u32) << 6,
            )
        };
    }
    #[inline(always)]
    pub fn set_suspend(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_SUSPEND_MASK) | (val as u32) << 7,
            )
        };
    }
    #[inline(always)]
    pub fn set_port_reset(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_PORT_RESET_MASK) | (val as u32) << 8,
            )
        };
    }
    #[inline(always)]
    pub fn set_port_power(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_PORT_POWER_MASK) | (val as u32) << 12,
            )
        };
    }
    #[inline(always)]
    pub fn set_port_owner(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_PORT_OWNER_MASK) | (val as u32) << 13,
            )
        };
    }
    #[inline(always)]
    pub fn set_port_test_control(&mut self, port_test_control: PortTestControl) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_PORT_TEST_CONTROL_MASK)
                    | port_test_control.as_u32() << 16,
            )
        };
    }
    #[inline(always)]
    pub fn set_wake_on_connect_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_WAKE_ON_CONNECT_ENABLE_MASK)
                    | (val as u32) << 20,
            )
        };
    }
    #[inline(always)]
    pub fn set_wake_on_disconnect_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_WAKE_ON_DISCONNECT_ENABLE_MASK)
                    | (val as u32) << 21,
            )
        };
    }
    #[inline(always)]
    pub fn set_wake_on_over_current_enable(&mut self, val: bool) {
        unsafe {
            self.address.write_volatile(
                self.address.read_volatile() & (!SET_WAKE_ON_OVER_CURRENT_ENABLE_MASK)
                    | (val as u32) << 22,
            )
        };
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
