use core::{ffi::c_uchar, slice};

use crate::{
    drivers::disk::{Disk, DiskIOResult},
    hal::{
        memory::allocator::{Allocator, MemoryBlock},
        print::{Module, simple_kernel_panic},
    },
    info,
    pfs::{EXT2_FS_SIGNATURE, gpt::GptPartitionEntry},
    utils::buffer::Buffer,
    vfs::gfs::{
        self, GFS_PERMISSIONS_EXECUTE, GFS_PERMISSIONS_WRITE, GeneralFileSystem,
        GfsReadError::{self, DiskError},
        GfsType,
        directory::GfsDirectory,
        directory_hashmap::GfsDirectoryHashmapManager,
        file::GfsFile,
        link::GfsLink,
    },
    warn,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ext2Superblock {
    pub total_number_inodes: u32,
    pub total_number_blocks: u32,
    pub superblock_reserved_blocks: u32,
    pub total_number_unallocated_blocks: u32,
    pub total_number_unallocated_inodes: u32,
    pub block_containing_superblock: u32,
    pub block_size: u32,
    pub fragment_size: u32,
    pub blocks_per_block_group: u32,
    pub fragments_per_block_group: u32,
    pub inodes_per_block_group: u32,
    pub last_mount_time: u32,
    pub last_written_time: u32,
    pub mounts_after_fsck: u16,
    pub mounts_per_fsck: u16,
    signature: u16,
    pub state: u16,
    pub error_handling_method: u16,
    pub minor_version: u16,
    pub fsck_last_time: u32,
    pub interval_per_fsck: u32,
    pub os_created_this_fs: u32,
    pub major_version: u32,
    pub reserved_blocks_usable_by_user: u16,
    pub reserved_blocks_usable_by_group: u16,
}
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Ext2ExtendedSuperblock {
    first_usable_inode: u32,
    size_of_inode: u16,
    block_group_of_this_superblock: u16,
    optional_features: u32,
    required_features: u32,
    readonly_non_present_features: u32,
    file_system_id: [u64; 2],
    volume_name: [c_uchar; 16],
    last_mounted_to: [c_uchar; 64],
    compression_algorithm: u32,
    preallocated_blocks_per_file: u8,
    preallocated_blocks_per_directory: u8,
    _unused: u16,
    journal_id: [u64; 2],
    journal_inode: u32,
    journal_device: u32,
    head_of_orphan_list: u32,
}
#[derive(Clone, Copy)]
pub struct Ext2Fs {
    pub superblock: Ext2Superblock,
    pub extended_superblock: Option<Ext2ExtendedSuperblock>,
    pub starting_lba: u64,
    pub disk_ident: u32,
}

pub enum LinuxFileSystem {
    Ext2 { data: Ext2Fs },
    Unregonized,
}

pub fn scan_linux_file_system(
    partition: &GptPartitionEntry,
    partition_id: usize,
    disk: &dyn Disk,
    allocator: &mut Allocator,
) -> LinuxFileSystem {
    let mut module = Module::new("LinuxPartition");
    let superblock_lba = partition.starting_lba + 1024 / disk.sector_size() as u64;
    let mut superblock_buf = Buffer::new(allocator, false, 1024);
    disk.read_into_buffer(superblock_lba, &superblock_buf, 1);

    if unsafe { *((superblock_buf.address() + 56) as *const u16) } == EXT2_FS_SIGNATURE {
        let superblock = *unsafe {
            (superblock_buf.address() as *const Ext2Superblock)
                .as_ref()
                .unwrap()
        };
        info!(
            &mut module,
            "Partition {} on Disk 0x{:x} is of type Ext2 {}.{}\n",
            partition_id,
            disk.identifier(),
            superblock.major_version,
            superblock.minor_version
        );
        let extended_superblock;
        if superblock.major_version >= 1 {
            extended_superblock = Option::Some(unsafe {
                *(((superblock_buf.address() + size_of::<Ext2Superblock>() as u64)
                    as *const Ext2ExtendedSuperblock)
                    .as_ref()
                    .unwrap())
            });
        } else {
            extended_superblock = Option::None;
        }
        let ret = LinuxFileSystem::Ext2 {
            data: Ext2Fs {
                superblock,
                extended_superblock,
                starting_lba: partition.starting_lba,
                disk_ident: disk.identifier(),
            },
        };
        superblock_buf.release(allocator);
        return ret;
    } else {
        superblock_buf.release(allocator);
        warn!(
            &mut module,
            "Partition {} on Disk 0x{:x} is unregonized\n",
            partition_id,
            disk.identifier()
        );
        return LinuxFileSystem::Unregonized;
    }
}

pub struct PhysicalExt2Directory {
    direct_pointers: [u32; 12],  /* u64 -> in Lba*/
    indirect_pointers: [u32; 3], /* u64 -> in Lba*/
    flags: u32,
    disk_sectors_used: u32,
    size: u32,
}

pub struct PhysicalExt2DirectoryEntry<'a> {
    pub inode: u32,
    pub total_size: u16,
    pub name_length: u8,
    pub type_indicator: u8,
    pub string: &'a str,
}

impl PhysicalExt2Directory {
    pub const fn empty() -> Self {
        return Self {
            direct_pointers: [0; 12],
            indirect_pointers: [0; 3],
            flags: 0x10,
            disk_sectors_used: 0,
            size: 0,
        };
    }

    pub fn foreach_entry<'a>(
        &self,
        allocator: &mut Allocator,
        disk: &dyn Disk,
        block_size: u16,
        starting_lba: u64,
        mut func: impl FnMut(&PhysicalExt2DirectoryEntry<'a>),
    ) {
        let dir_data = self.direct_pointers[0];
        if self.size > (1024 << block_size) {
            simple_kernel_panic("PhysicalExt2", "Root directory is bigger than 1 block\n");
        }

        let mut directory_data = Buffer::new(allocator, false, 1024 << block_size);
        disk.read_into_buffer(
            starting_lba + (dir_data as u64 * (1024 << block_size)) / disk.sector_size() as u64,
            &directory_data,
            1,
        );

        let mut current_ptr = directory_data.address();
        let mut size_rem = self.size;
        while size_rem != 0 {
            let inode = unsafe { *(current_ptr as *const u32) };
            let total_size = unsafe { *((current_ptr + 4) as *const u16) };
            let name_length = unsafe { *((current_ptr + 6) as *const u8) };
            let type_indicator = unsafe { *((current_ptr + 7) as *const u8) };
            let name = current_ptr + 8;

            (func)(&PhysicalExt2DirectoryEntry {
                inode,
                total_size,
                name_length,
                type_indicator,
                string: unsafe {
                    str::from_utf8_unchecked(slice::from_raw_parts(
                        name as *const u8,
                        name_length as usize,
                    ))
                },
            });
            current_ptr += total_size as u64;
            assert!(current_ptr % 4 == 0);
            size_rem -= total_size as u32;
        }
        directory_data.release(allocator);
    }

    pub fn contains(
        &self,
        allocator: &mut Allocator,
        disk: &dyn Disk,
        block_size: u16,
        starting_lba: u64,
        to_check: &str,
    ) -> Option<(u32, u8)> {
        let dir_data = self.direct_pointers[0];
        if self.size > (1024 << block_size) {
            simple_kernel_panic("PhysicalExt2", "Root directory is bigger than 1 block\n");
        }

        let mut directory_data = Buffer::new(allocator, false, 1024 << block_size);
        disk.read_into_buffer(
            starting_lba + (dir_data as u64 * (1024 << block_size)) / disk.sector_size() as u64,
            &directory_data,
            1,
        );

        let mut current_ptr = directory_data.address();
        let mut size_rem = self.size;
        while size_rem != 0 {
            let inode = unsafe { *(current_ptr as *const u32) };
            let total_size = unsafe { *((current_ptr + 4) as *const u16) };
            let name_length = unsafe { *((current_ptr + 6) as *const u8) };
            let type_indicator = unsafe { *((current_ptr + 7) as *const u8) };
            let name = current_ptr + 8;
            let string = unsafe {
                str::from_utf8_unchecked(slice::from_raw_parts(
                    name as *const u8,
                    name_length as usize,
                ))
            };
            if string.len() == to_check.len() {
                if string == to_check {
                    directory_data.release(allocator);
                    return Option::Some((inode, type_indicator));
                }
            }
            current_ptr += total_size as u64;
            assert!(current_ptr % 4 == 0);
            size_rem -= total_size as u32;
        }
        directory_data.release(allocator);
        return Option::None;
    }
}

/* Todo: this!*/
#[repr(C, align(4))]
#[derive(Clone, Copy)]
pub struct PhysicalExt2Inode {
    type_and_permissions: u16,
    user_id: u16,
    pub low_size: u32,
    last_access_size: u32,
    creation_time: u32,
    last_modification_time: u32,
    deletion_time: u32,
    group_id: u16,
    hard_links_count: u16,
    disk_sectors_used: u32,
    flags: u32,
    os_specific: u32,
    pub direct_block_pointers: [u32; 12],
    pub singly_indirect_block_pointer: u32,
    pub doubly_indirect_block_pointer: u32,
    pub triply_indirect_block_pointer: u32,
    generation_number: u32,
    reserved: [u32; 2],
    fragment_block_address: u32,
    os_specific_value: [u32; 3],
}

#[derive(Clone, Copy)]
pub struct PhysicalExt2BlockGroup {
    block_of_block_usage_bitmap: u32,
    block_of_inode_usage_bitmap: u32,
    block_address_of_inode_table: u32,
    number_unallocated_blocks: u16,
    number_unallocated_inodes: u16,
    directory_count: u16,
}

pub struct PhysicalExt2<'a> {
    memory: [MemoryBlock; 1],
    block_groups: &'a [PhysicalExt2BlockGroup],
    root_directory: PhysicalExt2Directory,
    starting_lba: u64,
    inodes_per_block_group: u32,
    block_size: u16,
}

/* INFO: this should be optimized*/
impl<'a> PhysicalExt2<'a> {
    pub fn block_group_of_inode(&self, inode: u32) -> u32 {
        (inode - 1) / self.inodes_per_block_group
    }
    pub fn index_into_block_group(&self, inode: u32) -> u32 {
        (inode - 1) % self.inodes_per_block_group
    }
    pub fn block_containing_inode(&self, index: u32) -> u32 {
        (index * 128) / (1024 << self.block_size)
    }
    pub fn root_contains(&self, allocator: &mut Allocator, disk: &dyn Disk, str: &str) -> bool {
        let ret =
            self.root_directory
                .contains(allocator, disk, self.block_size, self.starting_lba, str);
        if let Option::Some((_, _)) = ret {
            return true;
        } else {
            return false;
        }
    }
    #[inline(always)]
    pub fn max_direct_data_pointer_memory(&self) -> u32 {
        return 12 * (1024 << self.block_size);
    }
    #[inline(always)]
    pub fn max_singly_indirect_block_pointer_memory(&self) -> u32 {
        return (1024 << self.block_size) / size_of::<u32>() as u32 * (1024 << self.block_size);
    }

    pub fn load_root(
        &self,
        disk: &dyn Disk,
        dst: &mut GfsDirectory<'static>,
        gfs: &mut GeneralFileSystem,
        physical_allocator: &mut Allocator,
        fs_index: u32,
    ) {
        let dir_data = self.root_directory.direct_pointers[0];

        if self.root_directory.size > (1024 << self.block_size) {
            simple_kernel_panic("PhysicalExt2", "Root directory is bigger than 1 block\n");
        }

        let mut directory_data = Buffer::new(physical_allocator, false, 1024 << self.block_size);
        disk.read_into_buffer(
            self.starting_lba
                + (dir_data as u64 * (1024 << self.block_size)) / disk.sector_size() as u64,
            &directory_data,
            1,
        );
        let mut current_ptr = directory_data.address();
        let mut size_rem = self.root_directory.size;
        while size_rem != 0 {
            let inode = unsafe { *(current_ptr as *const u32) };
            let total_size = unsafe { *((current_ptr + 4) as *const u16) };
            let name_length = unsafe { *((current_ptr + 6) as *const u8) };
            let type_indicator = unsafe { *((current_ptr + 7) as *const u8) };
            let name_data = current_ptr + 8;

            let name = unsafe {
                str::from_utf8_unchecked(slice::from_raw_parts(
                    name_data as *const u8,
                    name_length as usize,
                ))
            };
            match type_indicator {
                1 /* regular file */=> {
                    let (file_index, file) = gfs.request_file();
                    self.load_inode_into_file(unsafe { &mut *file }, disk, physical_allocator, fs_index, inode);
                    dst.new_link(name, GfsLink::new(file_index, GfsType::File), unsafe { &mut *gfs.get_manager() });
                }
                2 /* directory*/ => {
                    let (mut directory_index, directory) = gfs.request_directory();
                    if name == "." || name == ".." {
                        unsafe {*directory = GfsDirectory::new(fs_index, 2, dst.get_entries_mut(),GFS_PERMISSIONS_WRITE)};
                        directory_index = gfs.linux_root_directory_index();
                    }else {
                        let inode_content = self.read_inode(inode, disk, physical_allocator);
                        let permissions;
                        if inode_content.type_and_permissions & 0x080 == 0{
                            permissions = 0;
                        }else {
                            permissions = GFS_PERMISSIONS_WRITE;
                        }
                        unsafe {*directory = GfsDirectory::new(fs_index, inode, &mut *gfs.request_hashmap(32).1, permissions)};
                    }
                    dst.new_link(name, GfsLink::new(directory_index, GfsType::Directory), unsafe { &mut *gfs.get_manager() });
                }
                _ => todo!("unhandeld type")
            }

            current_ptr += total_size as u64;
            assert!(current_ptr % 4 == 0);
            size_rem -= total_size as u32;
        }
        directory_data.release(physical_allocator);
    }

    pub fn read_inode(
        &self,
        inode: u32,
        disk: &dyn Disk,
        physical_allocator: &mut Allocator,
    ) -> PhysicalExt2Inode {
        let block_group = self.block_group_of_inode(inode);
        let index = self.index_into_block_group(inode);
        let block_address = self.block_groups[block_group as usize].block_address_of_inode_table;
        let inode_block = self.block_containing_inode(index);
        let base_inode = block_group * self.inodes_per_block_group
            + (inode_block * (1024 << self.block_size) / 128)
            + 1;
        let inode_offset = inode - base_inode;
        let lba = self.starting_lba
            + ((block_address + inode_block) as u64 * (1024 << self.block_size))
                / disk.sector_size() as u64;
        let mut inode_buffer =
            Buffer::new(physical_allocator, false, (inode_offset + 1) as u64 * 128);
        disk.read_into_buffer(lba, &inode_buffer, 1);
        let ret = unsafe {
            *((inode_buffer.address() + inode_offset as u64 * 128) as *const PhysicalExt2Inode)
        };
        inode_buffer.release(physical_allocator);
        return ret;
    }

    pub fn load_inode_into_file(
        &self,
        file: &mut GfsFile,
        disk: &dyn Disk,
        physical_allocator: &mut Allocator,
        fs_index: u32,
        inode_index: u32,
    ) {
        let inode_content = self.read_inode(inode_index, disk, physical_allocator);

        let mut permissions = 0;
        if inode_content.type_and_permissions & 0x0080 != 0 {
            permissions |= GFS_PERMISSIONS_WRITE;
        }
        if inode_content.type_and_permissions & 0x0100 != 0 {
            permissions |= GFS_PERMISSIONS_EXECUTE;
        }

        *file = GfsFile::new(
            fs_index,
            inode_index,
            inode_content.direct_block_pointers,
            inode_content.singly_indirect_block_pointer,
            inode_content.doubly_indirect_block_pointer,
            inode_content.triply_indirect_block_pointer,
            inode_content.low_size as u64,
            permissions,
        );
    }

    pub fn discover_directory(
        &self,
        fs_index: u32,
        gfs: &mut GeneralFileSystem,
        owner: &mut GfsDirectory,
        disk: &dyn Disk,
        physical_allocator: &mut Allocator,
        inode_index: u32,
        scheduler: u8,
    ) -> Option<GfsReadError> {
        let inode_raw = self.read_inode(inode_index, disk, physical_allocator);

        let mut directory_data = Buffer::new(physical_allocator, false, inode_raw.low_size as u64);

        match self.read_pointer_content_into_buffer(
            &directory_data,
            &inode_raw.direct_block_pointers,
            disk,
            scheduler,
        ) {
            Ok(_) => {}
            Err(e) => {
                directory_data.release(physical_allocator);
                return Option::Some(e);
            }
        };

        if inode_raw.low_size > self.max_direct_data_pointer_memory() {
            match self.read_singly_indirect_pointer_block(
                &directory_data
                    .sub_buffer(
                        self.max_direct_data_pointer_memory() as u64,
                        directory_data.get_size() - self.max_direct_data_pointer_memory() as u64,
                    )
                    .unwrap(),
                physical_allocator,
                inode_raw.singly_indirect_block_pointer,
                disk,
                scheduler,
            ) {
                Ok(_) => {}
                Err(e) => {
                    directory_data.release(physical_allocator);
                    return Option::Some(e);
                }
            }
        }

        if inode_raw.low_size
            > self.max_direct_data_pointer_memory()
                + self.max_singly_indirect_block_pointer_memory()
        {
            todo!("implement doubly/triply indirect\n");
        }

        let mut current_ptr = directory_data.address();
        let mut size_rem = self.root_directory.size;
        while size_rem != 0 {
            let inode = unsafe { *(current_ptr as *const u32) };
            let total_size = unsafe { *((current_ptr + 4) as *const u16) };
            let name_length = unsafe { *((current_ptr + 6) as *const u8) };
            let type_indicator = unsafe { *((current_ptr + 7) as *const u8) };
            let name_data = current_ptr + 8;

            let name = unsafe {
                str::from_utf8_unchecked(slice::from_raw_parts(
                    name_data as *const u8,
                    name_length as usize,
                ))
            };

            match owner
                .get_entries()
                .lookup(unsafe { &*gfs.get_manager() }, name)
            {
                Some(_) => {}
                None => {
                    let link;
                    match type_indicator {
                        1 /* regular file */=> {
                            let (index, file) = gfs.request_file();
                            self.load_inode_into_file(unsafe { &mut *file }, disk, physical_allocator, fs_index, inode);
                            link = GfsLink::new(index, GfsType::File);
                        }
                        2 /* directory*/=>  {
                            let (index, directory) = gfs.request_directory();

                            let inode_content = self.read_inode(inode, disk, physical_allocator);

                            let permissions;
                            if inode_content.type_and_permissions & 0x080 == 0{
                                permissions = 0;
                            }else {
                                permissions = GFS_PERMISSIONS_WRITE;
                            }

                            unsafe {
                                *directory = GfsDirectory::new(fs_index, inode , &mut *gfs.request_hashmap(32).1, permissions);
                            }

                            link = GfsLink::new(index, GfsType::Directory);
                        }
                        _ => todo!("implement other type insertion for discovery")
                    }

                    owner
                        .get_entries_mut()
                        .insert(unsafe { &mut *gfs.get_manager() }, name, link);

                    /* is not present*/
                }
            }
            current_ptr += total_size as u64;
            assert!(current_ptr % 4 == 0);
            size_rem -= total_size as u32;
        }

        directory_data.release(physical_allocator);
        return Option::None;
    }

    pub fn directory_from_inode(
        &self,
        allocator: &mut Allocator,
        inode: u32,
        disk: &dyn Disk,
    ) -> PhysicalExt2Directory {
        let inode_ = self.read_inode(inode, disk, allocator);
        return PhysicalExt2Directory {
            direct_pointers: inode_.direct_block_pointers,
            indirect_pointers: [
                inode_.singly_indirect_block_pointer,
                inode_.doubly_indirect_block_pointer,
                inode_.triply_indirect_block_pointer,
            ],
            flags: inode_.flags,
            disk_sectors_used: inode_.disk_sectors_used,
            size: inode_.low_size,
        };
    }

    pub fn load(
        superblock: &'a Ext2Superblock,
        extended_superblock: Option<&Ext2ExtendedSuperblock>,
        disk: &dyn Disk,
        allocator: &mut Allocator,
        starting_lba: u64,
    ) -> Self {
        if let Some(ext_block) = extended_superblock {
            if ext_block.required_features & 0x2 == 0 {
                simple_kernel_panic("PhysicalExt2", "Directory entries do not support type\n");
            }
            if ext_block.size_of_inode != 128 {
                simple_kernel_panic("PhysicalExt2", "Size of an Inode is not 128 bytes\n");
            }
        }

        let mut num_block_groups =
            superblock.total_number_inodes / superblock.inodes_per_block_group;
        if num_block_groups * superblock.inodes_per_block_group != superblock.total_number_inodes {
            num_block_groups += 1;
        }
        let memory_used = num_block_groups * size_of::<PhysicalExt2BlockGroup>() as u32;
        let mut pages_used = memory_used / 0x1000;
        if pages_used * size_of::<PhysicalExt2BlockGroup>() as u32 != memory_used {
            pages_used += 1;
        }
        let block_group_memory = allocator.alloc_zero(pages_used as u16).unwrap();
        let block_groups: &mut [PhysicalExt2BlockGroup] = unsafe {
            slice::from_raw_parts_mut(block_group_memory.as_mut_ptr(), num_block_groups as usize)
        };
        let mut block_group_descriptor_table_buf =
            Buffer::new(allocator, false, num_block_groups as u64 * 32);
        let full_block_size = 1024 << superblock.block_size;
        let block_group_descriptor_table_lba;
        if full_block_size == 1024 {
            block_group_descriptor_table_lba =
                starting_lba + (1024 * 3) / disk.sector_size() as u64; // block 2 => really value of 3
        } else {
            block_group_descriptor_table_lba =
                starting_lba + full_block_size / disk.sector_size() as u64;
        }
        disk.read_into_buffer(
            block_group_descriptor_table_lba,
            &block_group_descriptor_table_buf,
            1,
        );
        for i in 0..num_block_groups {
            let block_group = &mut block_groups[i as usize];
            *block_group = unsafe {
                *((block_group_descriptor_table_buf.address() + i as u64 * 32)
                    as *mut PhysicalExt2BlockGroup)
            };
        }
        block_group_descriptor_table_buf.release(allocator);

        let mut ret = Self {
            memory: [block_group_memory],
            block_groups: block_groups,
            root_directory: PhysicalExt2Directory::empty(),
            starting_lba,
            block_size: superblock.block_size as u16,
            inodes_per_block_group: superblock.inodes_per_block_group,
        };
        ret.root_directory = ret.directory_from_inode(allocator, 2, disk);
        return ret;
    }

    /**
     * Pointers must be direct pointers
     */
    pub fn read_pointer_content_into_buffer(
        &self,
        buffer: &Buffer,
        pointers: &[u32],
        disk: &dyn Disk,
        scheduler: u8,
    ) -> Result<u64, GfsReadError> {
        let mut iter = pointers.iter().peekable();
        let mut base = pointers[0];
        let mut series = 0;
        let mut offset = 0;
        while let Option::Some(pointer) = iter.next() {
            /* reads side by side blocks as one big read, which should hopefully increase performance*/
            if base + series != *pointer || (iter.peek().is_none() && series != 0) {
                let res = disk.read_into_buffer(
                    self.starting_lba
                        + (base * (1024 << self.block_size) / disk.sector_size()) as u64,
                    &buffer
                        .sub_buffer(offset, series as u64 * 1024 << self.block_size)
                        .unwrap(),
                    scheduler,
                );

                match res {
                    DiskIOResult::InvalidSize
                    | DiskIOResult::BufferReadonly
                    | DiskIOResult::InvalidLba
                    | DiskIOResult::InvalidScheduler => {
                        return Result::Err(DiskError(res));
                    }
                    DiskIOResult::Success => {}
                }

                base += *pointer;
                offset += series as u64 * 1024 << self.block_size as u64;
                if *pointer == 0 {
                    return Result::Ok(offset);
                }
                series = 0;
            } else {
                series += 1;
            }
        }
        return Result::Ok(offset);
    }

    pub fn read_singly_indirect_pointer_block(
        &self,
        buffer: &Buffer,
        allocator: &mut Allocator,
        block_pointer: u32,
        disk: &dyn Disk,
        scheduler: u8,
    ) -> Result<u64, GfsReadError> {
        let mut pointer_buffer = Buffer::new(allocator, false, 1024 << self.block_size);
        let mut res = disk.read_into_buffer(
            self.starting_lba
                + block_pointer as u64 * (1024 << self.block_size) / disk.sector_size() as u64,
            &pointer_buffer,
            scheduler,
        );

        match res {
            DiskIOResult::InvalidSize
            | DiskIOResult::BufferReadonly
            | DiskIOResult::InvalidLba
            | DiskIOResult::InvalidScheduler => {
                pointer_buffer.release(allocator);
                return Result::Err(DiskError(res));
            }
            DiskIOResult::Success => {}
        }

        let raw_pointers = unsafe {
            slice::from_raw_parts(
                pointer_buffer.as_const() as *const u32,
                (1024 << self.block_size) / 4,
            )
        };
        let mut pointers = raw_pointers.iter().peekable();

        let mut base = raw_pointers[0];
        let mut series = 0;
        let mut offset = 0u64;
        while let Option::Some(pointer) = pointers.next() {
            /* reads when the end of the list is reached or when the next pointer is zero
             * also combines multiply pointers which are side by sides into one big pointer
             * When pointer = null, this will succeed thus reading
             */
            if *pointer != base + series || (pointers.peek().is_none() && series != 0) {
                res = disk.read_into_buffer(
                    self.starting_lba
                        + base as u64 * (1024 << self.block_size) / disk.sector_size() as u64,
                    &buffer
                        .sub_buffer(offset, series as u64 * 1024 << self.block_size)
                        .unwrap(),
                    scheduler,
                );
                match res {
                    DiskIOResult::InvalidSize
                    | DiskIOResult::BufferReadonly
                    | DiskIOResult::InvalidLba
                    | DiskIOResult::InvalidScheduler => {
                        pointer_buffer.release(allocator);
                        return Result::Err(DiskError(res));
                    }
                    DiskIOResult::Success => {}
                }
                offset += series as u64 * 1024 << self.block_size;
                if *pointer == 0 {
                    return Result::Ok(offset);
                }
                base = *pointer;
                series = 0;
            } else {
                series += 1;
            }
        }
        pointer_buffer.release(allocator);
        return Result::Ok(offset);
    }
}
