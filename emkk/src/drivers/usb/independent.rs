use crate::{
    drivers::usb::{
        independent::UsbSpeed::{FullSpeed, LowSpeed, SuperSpeedPlusGen2x2},
        standard_requests::UsbDeviceDescriptor,
    },
    hal::print::simple_kernel_panic,
};

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
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Direction {
    In,
    Out,
    TdDependent,
    Invalid,
}

impl Direction {
    pub fn from_bool(val: bool) -> Direction {
        return match val {
            false => Direction::Out,
            true => Direction::In,
        };
    }

    pub fn from_ohci(val: u32) -> Self {
        return match val {
            0b00 | 0b11 => Self::TdDependent,
            0b01 => Self::Out,
            0b10 => Self::In,
            _ => simple_kernel_panic("Direction/from_ohci", "Invalid value\n"),
        };
    }

    pub fn as_ohci(&self) -> u32 {
        return match self {
            Self::Out => 0b01,
            Self::In => 0b10,
            Self::TdDependent => 0b11,
            Self::Invalid => simple_kernel_panic("Direction/as_ohci", "Self is Invalid\n"),
        };
    }

    pub fn as_ehci(&self) -> u32 {
        return match self {
            Self::In => 1,
            Self::Out => 0,
            Self::TdDependent => simple_kernel_panic(
                "Direction/as_ehci",
                "Ehci does not support TdDependent Direction\n",
            ),
            Self::Invalid => simple_kernel_panic("Direction/as_ehci", "Self Is Invalid\n"),
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
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum UsbDescriptorType {
    /* USB 1.0 Forward*/
    Device = 1,
    Configuration = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
    /* USB 2.0*/
    DeviceQualifier = 6,
    OtherSpeedConfiguration = 7,
    InterfacePower = 8,
}
#[repr(u8)]
pub enum UsbRequestCode {
    GetStatus = 0,
    ClearFeature = 1,
    SetFeature = 3,
    SetAddress = 5,
    GetDescriptor = 6,
    SetDescriptor = 7,
    GetConfiguration = 8,
    SetConfiguration = 9,
    GetInterface = 10,
    SetInterface = 11,
    SynchFrame = 12,
}

pub enum UsbFeatureSelector {
    /* USB 1.0 Forward*/
    DeviceRemoteWakeup = 1,
    EndpointHalt = 0,
    /* USB 2.0*/
    TestMode = 2,
}

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
    pub max_power_ma: u16,
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

    pub fn from_descriptor(descriptor: &UsbDeviceDescriptor) -> Self {
        return Self {
            device_class: descriptor.b_device_class,
            device_sub_class: descriptor.b_device_sub_class,
            device_protocol: descriptor.b_device_protocol,
            vendor_id: descriptor.id_vendor,
            product_id: descriptor.id_product,
            manufacturer: descriptor.i_manufacturer,
            i_product: descriptor.i_product,
            serial_number: descriptor.i_serial_number,
            max_power_ma: 0,
            num_interfaces: 0,
        };
    }
}
#[derive(Clone, Copy)]
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
#[repr(u8)]
#[derive(Clone, Copy)]
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

pub enum PidCode {
    OutToken,
    InToken,
    SetupToken,
}

impl PidCode {
    pub fn from_ehci(val: u32) -> Self {
        return match val {
            0 => Self::OutToken,
            1 => Self::InToken,
            2 => Self::SetupToken,
            _ => simple_kernel_panic("PidCode/from_ehci", "Invalid value\n"),
        };
    }
    pub fn from_ohci(val: u32) -> Self {
        return match val {
            0 => Self::SetupToken,
            1 => Self::OutToken,
            2 => Self::InToken,
            _ => simple_kernel_panic("PidCode/from_ohci", "Invalid value\n"),
        };
    }
    pub fn as_ehci(&self) -> u32 {
        return match self {
            Self::OutToken => 0,
            Self::InToken => 1,
            Self::SetupToken => 2,
        };
    }
    pub fn as_ohci(&self) -> u32 {
        return match self {
            Self::SetupToken => 0,
            Self::OutToken => 1,
            Self::InToken => 2,
        };
    }
    // true  => In
    // false => Out
    pub fn inout_from_bool(val: bool) -> Self {
        return match val {
            true => Self::InToken,
            false => Self::OutToken,
        };
    }
}

pub enum UsbSpeed {
    LowSpeed,
    FullSpeed,
    HighSpeed,
    SuperSpeedGen1x1,
    SuperSpeedPlusGen2x1,
    SuperSpeedPlusGen1x2,
    SuperSpeedPlusGen2x2,
}
impl UsbSpeed {
    pub fn as_str(&self) -> &'static str {
        return match self {
            Self::LowSpeed => "Low Speed",
            Self::FullSpeed => "Full Speed",
            Self::HighSpeed => "High Speed",
            Self::SuperSpeedGen1x1 => "Super Speed 1.1",
            Self::SuperSpeedPlusGen2x1 => "Super Speed+ 2.1",
            Self::SuperSpeedPlusGen1x2 => "Super Speed+ 1.2",
            Self::SuperSpeedPlusGen2x2 => "Super Speed+ 2.2",
        };
    }

    pub fn max_packet_size_for_device_address(&self) -> u16 {
        return match self {
            Self::LowSpeed | Self::FullSpeed => 8,
            Self::HighSpeed => 64,
            Self::SuperSpeedGen1x1
            | Self::SuperSpeedPlusGen2x1
            | Self::SuperSpeedPlusGen1x2
            | SuperSpeedPlusGen2x2 => 512,
        };
    }
}

pub enum UsbProtocol {
    Usb1,
    Usb2,
    Usb3,
    Usb3_1,
    Usb3_2,
}

pub const USB_MICRO_FRAME_TO_FRAME_CONVERSION_FACTOR: u16 = 8;
