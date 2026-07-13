use core::ffi::{c_uchar, c_void};

use crate::hal::print::simple_kernel_panic;

#[repr(C, packed)]
pub struct XhciExtendedCapability {
    pub capability_id: u8,
    pub next_ptr: u8,
    pub capability_specific: u16,
}
#[repr(align(4))]
pub struct XhciProtocolDefinition {
    pub dword0: u32,
}

pub enum XhciProtocolDefinitionPart {
    /** Protocol Speed ID Value*/
    Psiv = (0xF << 16) | 0,
    /** Protocol Speed ID Exponent*/
    Psie = (0x3 << 16) | 4,
    /** PSI Type*/
    Plt = (0x3 << 16) | 6,
    /** Link Protocol*/
    Lp = (0x3 << 16) | 14,
    /** Protocol Speed ID Mantissa*/
    Psim = (0xFFFF << 16) | 16,
}

impl XhciProtocolDefinition {
    pub fn get(&self, what_to_get: XhciProtocolDefinitionPart) -> u32 {
        let get_u32 = what_to_get as u32;
        return (self.dword0 >> (get_u32 & 0xFF)) & (get_u32 >> 16);
    }
    /** PSI Full-duplex*/
    pub fn pfd(&self) -> bool {
        return 0 != self.dword0 & 1 << 8;
    }
}

pub struct XhciSupportedProtocol {
    pub name_string: [c_uchar; 4],
    pub revision_minor: u8,
    pub revision_major: u8,
    pub compatible_port_offset: u8,
    pub compatible_port_count: u8,
    pub protcol_defined: u16,
    pub psic: u8,
    pub protocol_slot_type: u8,
    pub definition_ptr: *const u32,
}

impl XhciSupportedProtocol {
    #[inline(always)]
    pub fn get_definition(&self, index: u8) -> XhciProtocolDefinition {
        return XhciProtocolDefinition {
            dword0: unsafe { *self.definition_ptr.add(index as usize) },
        };
    }
}
#[repr(u32)]
pub enum XhciTrbId {
    Normal = 1,
    Setup = 2,
    Data = 3,
    Status = 4,
    Isoch = 5,
    Link = 6,
    EventData = 7,
    NoOp = 8,
    EnableSlotCommand = 9,
    DisableSlotCommand = 10,
    AddressDeviceCommand = 11,
    ConfigureEndpointCommand = 12,
    EvaluateContextCommand = 13,
    ResetEndpointCommand = 14,
    StopEndpointCommand = 15,
    SetTRDequeuePointerCommand = 16,
    ResetDeviceCommand = 17,
    ForceHeaderCommand = 22,
    NoOpCommandTrb = 23,
    GetExtendedPropertyCommand = 24,
    SetExtendedPropertyCommand = 25,
    TransferEvent = 32,
    CommandCompletionEvent = 33,
    PortStatusChangeEvent = 34,
    HostControllerEvent = 37,
    DeviceNotificationEvent = 38,
    MFindexWrapEvent = 39,
}

impl XhciTrbId {
    pub fn from_u32(r#in: u32) -> Self {
        match r#in {
            1 => XhciTrbId::Normal,
            2 => XhciTrbId::Setup,
            3 => XhciTrbId::Data,
            4 => XhciTrbId::Status,
            5 => XhciTrbId::Isoch,
            6 => XhciTrbId::Link,
            7 => XhciTrbId::EventData,
            8 => XhciTrbId::NoOp,
            9 => XhciTrbId::EnableSlotCommand,
            10 => XhciTrbId::DisableSlotCommand,
            11 => XhciTrbId::AddressDeviceCommand,
            12 => XhciTrbId::ConfigureEndpointCommand,
            13 => XhciTrbId::EvaluateContextCommand,
            14 => XhciTrbId::ResetEndpointCommand,
            15 => XhciTrbId::StopEndpointCommand,
            16 => XhciTrbId::SetTRDequeuePointerCommand,
            17 => XhciTrbId::ResetDeviceCommand,
            22 => XhciTrbId::ForceHeaderCommand,
            23 => XhciTrbId::NoOpCommandTrb,
            24 => XhciTrbId::GetExtendedPropertyCommand,
            25 => XhciTrbId::SetExtendedPropertyCommand,
            32 => XhciTrbId::TransferEvent,
            33 => XhciTrbId::CommandCompletionEvent,
            34 => XhciTrbId::PortStatusChangeEvent,
            37 => XhciTrbId::HostControllerEvent,
            38 => XhciTrbId::DeviceNotificationEvent,
            39 => XhciTrbId::MFindexWrapEvent,
            _ => simple_kernel_panic("XhciTrbId/from_u32", "Invalid in value\n"),
        }
    }
}
