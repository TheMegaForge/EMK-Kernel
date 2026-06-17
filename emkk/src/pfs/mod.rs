use crate::{hal::memory::allocator::MemoryBlock, pfs::linux::Ext2Fs};

pub mod disk_scanner;
pub mod gpt;
pub mod linux;

pub const EFI_SYSTEM_PARTITION_GUID0: u64 = 0x11D2F81FC12A7328; // 28 73 2A C1 1F F8 D2 11
pub const EFI_SYSTEM_PARTITION_GUID1: u64 = 0x3BC93EC9A0004BBA; // BA 4B 00 A0 C9 3E C9 3B

pub const LINUX_FILE_SYSTEM_PARTITION_GUID0: u64 = 0x477284830FC63DAF; // AF 3D C6 0F 83 84 72 47
pub const LINUX_FILE_SYSTEM_PARTITION_GUID1: u64 = 0xE47D47D8693D798E; // 8E 79 3D 69 D8 47 7D E4

pub const EXT2_FS_SIGNATURE: u16 = 0xef53;

pub struct DiskFileSystemArray<'a> {
    pub num_total: u8,
    pub num_ext2: u8,
    pub _num_ext3: u8,
    pub _num_ext4: u8,
    pub _num_ntfs: u8,
    pub _num_fat16: u8,
    pub _num_fat32: u8,
    pub _num_exfat: u8,

    pub file_system_data: MemoryBlock,

    pub ext2: &'a [Ext2Fs],
    // _ext3: &[Ext3Fs]
    // _ext4: &[Ext4Fs]
    // _ntfs: &[WinNTFs]
    // _fat16: &[Fat16Fs]
    // _fat32: &[Fat32Fs]
    // _exfat: &[ExFatFs]
}
