use crate::drivers::usb::uhci::structures::endpoint::UhciControlEndpoint;

pub mod device;
pub mod endpoint;
pub mod frame_list;
pub mod interface;
pub(in crate::drivers::usb::uhci) static mut UHCI_DUMMY_CONTROL_ENDPOINT: UhciControlEndpoint =
    UhciControlEndpoint::empty();

pub const QUEUE_HEAD_WAS_INTERRUPT: u32 = 1 << 29;

pub const QUEUE_HEAD_WAS_CONTROL: u32 = 1 << 30;
pub const QUEUE_HEAD_CONTROL_SIMPLE: u32 = 1 << 29;
pub const QUEUE_HEAD_WAS_CUSTOM: u32 = 1 << 31;
