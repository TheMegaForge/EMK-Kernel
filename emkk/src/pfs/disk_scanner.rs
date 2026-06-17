use core::slice;

use crate::{
    drivers::disk::{DiskController, nvme::NVMeController, sata::SataController},
    hal::{memory::allocator::Allocator, print::Module},
    info,
    pfs::{
        DiskFileSystemArray, EFI_SYSTEM_PARTITION_GUID0, EFI_SYSTEM_PARTITION_GUID1,
        LINUX_FILE_SYSTEM_PARTITION_GUID0, LINUX_FILE_SYSTEM_PARTITION_GUID1,
        gpt::Gpt,
        linux::{LinuxFileSystem, scan_linux_file_system},
    },
    utils::allocators::PageAllocator,
};

pub fn scan_disks<'a>(
    allocator: &mut Allocator,
    nvme_controller: Option<&NVMeController>,
    sata_controller: Option<&SataController>,
) -> DiskFileSystemArray<'a> {
    let mut module = Module::new("Disk Scanner");
    let mut ext2_allocator = PageAllocator::new(allocator, 100);

    if let Some(nvme_cntrl) = nvme_controller {
        for d in 0..nvme_cntrl.num_disks_present() {
            match nvme_cntrl.get_disk_indexed(d) {
                Some((ident, disk)) => {
                    let mut gpt = Gpt::new(disk, allocator);

                    for (partition_id, partition) in gpt.gpt_entries.iter().enumerate() {
                        if partition.partition_guid[0] == EFI_SYSTEM_PARTITION_GUID0
                            && partition.partition_guid[1] == EFI_SYSTEM_PARTITION_GUID1
                        {
                            info!(&mut module, "Efi System Partition on Disk 0x{:x}\n", ident);
                        } else if partition.partition_guid[0] == LINUX_FILE_SYSTEM_PARTITION_GUID0
                            && partition.partition_guid[1] == LINUX_FILE_SYSTEM_PARTITION_GUID1
                        {
                            info!(&mut module, "Linux File System on Disk 0x{:x}\n", ident);
                            let lfs =
                                scan_linux_file_system(partition, partition_id, disk, allocator);
                            match lfs {
                                LinuxFileSystem::Ext2 { data } => {
                                    ext2_allocator.push_back(data);
                                }
                                LinuxFileSystem::Unregonized => {}
                            }
                        }
                    }
                    gpt.release(allocator);
                }
                None => {}
            }
        }
    }

    let file_system_data = allocator.alloc_zero(1).unwrap();
    let ext2_fs_block = unsafe {
        slice::from_raw_parts_mut(
            file_system_data.as_mut_ptr(),
            ext2_allocator.size() as usize,
        )
    };
    ext2_allocator.for_each(|i, fs| {
        ext2_fs_block[i as usize] = unsafe { *fs };
        true
    });
    ext2_allocator.free(allocator);

    let ret = DiskFileSystemArray {
        num_total: ext2_fs_block.len() as u8,
        num_ext2: ext2_fs_block.len() as u8,
        _num_ext3: 0,
        _num_ext4: 0,
        _num_ntfs: 0,
        _num_fat16: 0,
        _num_fat32: 0,
        _num_exfat: 0,
        file_system_data,
        ext2: ext2_fs_block,
    };

    return ret;
}
