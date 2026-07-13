pub mod device;
pub mod endpoint;
pub mod interface;
pub mod interrupt_list;
pub mod non_periodic_list;
pub mod transfer_descriptors;

pub const OHCI_TRANSFER_DESCRIPTOR_PROCESSED: u32 = 1 << 31;
