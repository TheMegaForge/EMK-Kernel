use core::slice;

use crate::{
    drivers::disk::Disk,
    hal::{memory::allocator::Allocator, print::simple_kernel_panic},
    utils::buffer::Buffer,
};

#[repr(C, packed)]
pub struct GptPartitionEntry {
    pub partition_guid: [u64; 2],
    pub unique_guid: [u64; 2],
    pub starting_lba: u64,
    pub ending_lba: u64,
    pub attributes: u64,
    pub partition_name: [u8; 72],
}

pub struct Gpt<'a> {
    _gpt_header: GptHeader,
    buffer: Buffer,
    pub gpt_entries: &'a [GptPartitionEntry],
}
#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct GptHeader {
    signature: u64,
    gpt_revision: u32,
    header_size: u32,
    crc32_checksum: u32,
    reserved: u32,
    this_lba: u64,
    alternate_lba: u64,
    first_usable_block_gpt_entry: u64,
    last_usable_block_gpt_entry: u64,
    guid: [u8; 16],
    partition_entry_array_starting_lba: u64,
    num_partition_entries: u32,
    partition_entry_size: u32,
    crc32: u32,
}

impl<'a> Gpt<'a> {
    pub fn new(disk: &dyn Disk, allocator: &mut Allocator) -> Self {
        let mut gpt_header = Buffer::new(allocator, false, 512);

        disk.read_into_buffer(1, &gpt_header, 1);
        let header = unsafe {
            (gpt_header.as_const() as *const GptHeader)
                .as_ref()
                .unwrap()
        };

        let size = header.num_partition_entries * header.partition_entry_size;
        if header.partition_entry_size != 128 {
            simple_kernel_panic("Gpt", "Partition Entry Size != 128 bytes\n");
        }
        let entry_array = Buffer::new(allocator, false, size as u64);
        disk.read_into_buffer(header.partition_entry_array_starting_lba, &entry_array, 1);

        let gpt_entries = unsafe {
            slice::from_raw_parts(
                entry_array.as_const() as *const GptPartitionEntry,
                header.num_partition_entries as usize,
            )
        };

        gpt_header.release(allocator);
        let gpt_header_ = header.clone();
        return Self {
            _gpt_header: gpt_header_,
            buffer: entry_array,
            gpt_entries,
        };
    }
    pub fn release(&mut self, allocator: &mut Allocator) {
        self.buffer.release(allocator);
    }
}
