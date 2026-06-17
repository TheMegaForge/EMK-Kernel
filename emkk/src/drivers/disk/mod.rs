use crate::{
    hal::{
        memory::{allocator::Allocator, pager::Pager},
        pci_bus::PciBus,
    },
    utils::buffer::Buffer,
};

pub mod nvme;
pub mod nvme_queue;
pub mod nvme_registers;
pub mod nvme_structures;
pub mod sata;
pub mod sata_abar;
pub mod sata_structures;
pub enum ControllerType {
    Sata,
    NVMe,
}
#[derive(Debug)]
pub enum DiskIOResult {
    Success,
    InvalidScheduler,
    InvalidLba,
    InvalidSize,
    BufferReadonly,
}
/*
 * FIXME: MAJOR FLAW.
 * Add offset field, so that when the sector at lba get´s read, only the data after offset get´s copied into the buffer
 */
pub trait Disk {
    fn read_into_buffer(&self, lba: u64, buffer: &Buffer, scheduler: u8) -> DiskIOResult;
    fn write_from_buffer(&self, lba: u64, buffer: &Buffer, scheduler: u8) -> DiskIOResult;
    fn sector_size(&self) -> u32;
    fn num_sectors(&self) -> u64;
    fn identifier(&self) -> u32;
}

pub trait DiskController {
    fn new(
        pci_bus: &PciBus,
        pci_device: u64,
        allocator: &mut Allocator,
        pager: &mut Pager,
        isr_vector: u8,
        dst: &mut Self,
    );
    fn present(&self) -> bool;

    fn identify(&self) -> ControllerType;
    fn num_disks_present(&self) -> u8;
    fn hotplugging_is_supported(&self) -> bool;

    fn get_disk_mut(&mut self, identifier: u32) -> Option<&'static mut dyn Disk>;
    fn get_disk_indexed_mut(&mut self, index: u8) -> Option<(u32, &'static mut dyn Disk)>;

    fn get_disk(&self, identifier: u32) -> Option<&'static dyn Disk>;
    fn get_disk_indexed(&self, index: u8) -> Option<(u32, &'static dyn Disk)>;
}
