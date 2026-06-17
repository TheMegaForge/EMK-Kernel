use crate::hal::print::simple_kernel_panic;

pub enum UsbEndpointError {
    MaximumQTDsExceeded,
}

pub enum UsbHidDeviceType {
    Keyboard,
    Mouse,
}

pub enum HciState {
    Running,
    Stopped,
}

impl HciState {
    pub fn from_bool(state: bool) -> HciState {
        return match state {
            true => HciState::Running,
            false => HciState::Stopped,
        };
    }
}

pub enum Direction {
    In,
    Out,
}

impl Direction {
    pub fn from_bool(val: bool) -> Direction {
        return match val {
            false => Direction::Out,
            true => Direction::In,
        };
    }
    pub fn as_u32(&self) -> u32 {
        return match self {
            Self::In => 1,
            Self::Out => 0,
        };
    }
}

#[derive(Clone, Copy)]
pub enum UsbDeviceState {
    /* Custom*/
    Invalid,
    Resetted,
    Detached,
    /* Specified */
    Default,
    Address,
    Configured,
}

pub enum UsbRecipient {
    Zero,
    Interface(u8),
    Endpoint(u8),
}

pub const DEVICE_DESCRIPTOR_TYPE: u8 = 1;
pub const CONFIGURATION_DESCRIPTOR_TYPE: u8 = 2;
pub const STRING_DESCRIPTOR_TYPE: u8 = 3;
pub const INTERFACE_DESCRIPTOR_TYPE: u8 = 4;
pub const ENDPOINT_DESCRIPTOR_TYPE: u8 = 5;
pub const DEVICE_QUALIFIER_DESCRIPTOR_TYPE: u8 = 6;
pub const OTHER_SPEED_CONFIGURATION_DESCRIPTOR_TYPE: u8 = 7;
pub const INTERFACE_POWER_DESCRIPTOR_TYPE: u8 = 8;

pub const GET_STATUS_REQUEST: u8 = 0;
pub const CLEAR_FEATURE_REQUEST: u8 = 1;
pub const SET_FEATURE_REQUEST: u8 = 3;
pub const SET_ADDRESS_REQUEST: u8 = 5;
pub const GET_DESCRIPTOR_REQUEST: u8 = 6;
pub const SET_DESCRIPTOR_REQUEST: u8 = 7;
pub const GET_CONFIGURATION_REQUEST: u8 = 8;
pub const SET_CONFIGURATION_REQUEST: u8 = 9;
pub const GET_INTERFACE_REQUEST: u8 = 10;
pub const SET_INTERFACE_REQUEST: u8 = 11;
pub const SYNCH_FRAME_REQUEST: u8 = 12;

pub const DEVICE_REMOTE_WAKEUP_FEATURE_SELECTOR: u8 = 1;
pub const ENDPOINT_HALT_FEATURE_SELECTOR: u8 = 0;
pub const TEST_MODE_FEATURE_SELECTOR: u8 = 2;

pub const BOOT_PROTOCOL: u16 = 0;

pub type UsbInterfaceAlternateSetting = u8;
pub type UsbEndpointFrameNumber = u16;
pub type UsbGeneralStatus = u16;

pub type UsbDeviceConfiguration = u8;
pub struct UsbDeviceInformation {
    pub device_class: u8,
    pub device_sub_class: u8,
    pub device_protocol: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: u8,
    pub i_product: u8,
    pub serial_number: u8,
    pub max_power_ma: u8,
    pub num_interfaces: u8,
}

impl UsbDeviceInformation {
    pub const fn empty() -> Self {
        return Self {
            device_class: 0,
            device_sub_class: 0,
            device_protocol: 0,
            vendor_id: 0,
            product_id: 0,
            manufacturer: 0,
            i_product: 0,
            serial_number: 0,
            max_power_ma: 0,
            num_interfaces: 0,
        };
    }
}

pub enum UsbControllerType {
    OHC,
    UHC,
    EHC,
    XHC,
}

pub struct UsbControllerInformation {
    pub active_device_count: u16,
    pub potential_device_count: u16,
}

impl UsbControllerInformation {
    pub const fn empty() -> Self {
        return Self {
            active_device_count: 0,
            potential_device_count: 0,
        };
    }
}

pub enum UsbTransferType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

impl UsbTransferType {
    pub fn from_u8(val: u8) -> Self {
        return match val {
            0 => Self::Control,
            1 => Self::Isochronous,
            2 => Self::Bulk,
            3 => Self::Interrupt,
            _ => simple_kernel_panic("UsbTransferType/from_u8", "Invalid val\n"),
        };
    }
}

pub const USB_MICRO_FRAME_TO_FRAME_CONVERSION_FACTOR: u16 = 8;
