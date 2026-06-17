use core::{
    alloc,
    ffi::{c_uchar, c_void},
    ops::Add,
    ptr::{null, slice_from_raw_parts_mut},
    slice,
};

use crate::{
    arch::lapic::LocalApic,
    drivers::disk::{
        Disk, DiskController, DiskIOResult, nvme::NVMeController, sata::SataController,
    },
    fixed_vaddrs::{
        FIXED_PROCESSOR_VIRTUAL_ADDRESS, GFS_DIRECTORY_FIXED_VADDR, GFS_FILES_FIXED_VADDR,
        GFS_HASHMAP_FIXED_VADDR, GFS_STRINGS_FIXED_VADDR, ref_processor_mut,
    },
    hal::{
        memory::{
            allocator::{Allocator, MemoryBlock},
            pager::{PAGER_PRESENT, PAGER_RW},
        },
        print::{Module, ModuleWriteMode::Warn, simple_kernel_panic},
    },
    info,
    multithreading::{Multithreading, processors::Processor},
    pfs::{
        disk_scanner::scan_disks,
        linux::{Ext2ExtendedSuperblock, Ext2Fs, Ext2Superblock, PhysicalExt2},
    },
    processes::IpiVMemSync,
    success,
    utils::{buffer::Buffer, to_lowercase},
    vfs::gfs::{
        GfsFsType::Ext2,
        directory::{GfsDirectory, GfsInsertion},
        directory_hashmap::{GfsDirectoryHashmap, GfsDirectoryHashmapManager},
        file::GfsFile,
        link::GfsLink,
    },
};

pub type GfsPermissions = u16;
pub type GfsState = u16;

pub const GFS_PERMISSIONS_WRITE: u16 = 0x0001;
pub const GFS_PERMISSIONS_VERIFIED_ACCESS: u16 = 0x0002;
pub const GFS_PERMISSIONS_SYSTEM_ACCESS: u16 = 0x0004;
pub const GFS_PERMISSIONS_EXECUTE: u16 = 0x0008;
/*
 * If occupation_map in File != 0 => opening fails
 */
pub const GFS_PERMISSIONS_EXCLUSIVE: u16 = 0x1000;
pub const GFS_PERMISSIONS_WRITE_SHARING: u16 = 0x2000;

pub const GFS_FLAG_CLOSED: u16 = 0x0000;
pub const GFS_FLAG_EXECUTING: u16 = 0x0001;
pub enum GfsType {
    File,
    Directory,
}

impl GfsType {
    pub fn to_u64(&self) -> u64 {
        return match self {
            Self::File => 1,
            Self::Directory => 2,
        };
    }
    pub fn from_u64(val: u64) -> Self {
        return match val {
            1 => Self::File,
            2 => Self::Directory,
            _ => simple_kernel_panic("GfsType", "Invalid Input\n"),
        };
    }
}

pub mod directory;
pub mod directory_hashmap;
pub mod file;
pub mod link;
pub mod string_allocator;
/* Windows partitions are name left to right ; Linux partitions are names right to left*/
static NTFS_ROOT_NAMES: [c_uchar; 24] = [
    'c' as u8, 'd' as u8, 'e' as u8, 'f' as u8, 'g' as u8, 'h' as u8, 'i' as u8, 'j' as u8,
    'k' as u8, 'l' as u8, 'm' as u8, 'n' as u8, 'o' as u8, 'p' as u8, 'q' as u8, 'r' as u8,
    's' as u8, 't' as u8, 'u' as u8, 'v' as u8, 'w' as u8, 'x' as u8, 'y' as u8, 'z' as u8,
];

pub static mut GFS: GeneralFileSystem = GeneralFileSystem::empty();
pub(in crate::vfs::gfs) static mut GFS_STRING_ALLOCATED_SIZE: u32 = 0x20000;
pub enum GfsFsType<'a> {
    Ext2 {
        superblock: &'a mut Ext2Superblock,
        extended_superblock: Option<&'a Ext2ExtendedSuperblock>,
        physical: &'a mut PhysicalExt2<'a>,
    },
    Nothing,
}

pub struct GfsFs<'a> {
    fs_type: GfsFsType<'a>,
    starting_lba: u64,
    file_system_size: u64,
    disk: &'a dyn Disk,
}

pub struct GfsNtfsRoot<'a> {
    file_system: &'a GfsFs<'a>,
    directory_index: u32,
    letter: c_uchar,
}

/* REFERENCE HELL!*/
/* FIXME?: every value inside think´s it will exist while the structure exists.
 * But this is wrong, since a hotplug event could occur and mess things up.
 */
/* TODO:*/

pub struct GfsLinuxRoot {
    directory_index: u32,
    fs_index: u32,
}

pub struct GeneralFileSystem<'a> {
    memory: [MemoryBlock; 3],
    detected_file_systems: &'a [GfsFs<'a>],
    fs_specific_data: *const c_void,
    ntfs_roots: &'a [Option<GfsNtfsRoot<'a>>],
    linux_root: GfsLinuxRoot, // pointer to a directory
    directories: &'static mut [GfsDirectory<'static>],
    files: &'static mut [GfsFile],
    hashmap_manager: GfsDirectoryHashmapManager<'a>,
    hashmaps: &'static mut [GfsDirectoryHashmap],
    files_allocated: u32,
    directories_allocated: u32,
    hashmaps_allocated: u32,
}
impl<'a> GeneralFileSystem<'a> {
    /* NOTICE: this will result in an corrupt and unusable GeneralFileSystem.
     * The only duty of this function is to initialize GFS before 'system_table.initialize_file_systems()' is called
     */
    pub const fn empty() -> Self {
        return Self {
            memory: [const { MemoryBlock::empty() }; 3],
            detected_file_systems: unsafe {
                slice::from_raw_parts(align_of::<GfsFs>() as *const GfsFs, 0)
            },
            fs_specific_data: null(),
            ntfs_roots: unsafe {
                slice::from_raw_parts(
                    align_of::<Option<GfsNtfsRoot>>() as *const Option<GfsNtfsRoot>,
                    0,
                )
            },
            linux_root: GfsLinuxRoot {
                directory_index: 0,
                fs_index: 0,
            },
            directories: unsafe {
                slice::from_raw_parts_mut(align_of::<GfsDirectory>() as *mut GfsDirectory, 0)
            },
            files: unsafe { slice::from_raw_parts_mut(align_of::<GfsFile>() as *mut GfsFile, 0) },
            hashmap_manager: GfsDirectoryHashmapManager::empty(),
            hashmaps: unsafe {
                slice::from_raw_parts_mut(
                    align_of::<GfsDirectoryHashmap>() as *mut GfsDirectoryHashmap,
                    0,
                )
            },
            directories_allocated: 0,
            files_allocated: 0,
            hashmaps_allocated: 0,
        };
    }

    fn send_sync_ipi(threading: &Multithreading, memory_block: MemoryBlock, vaddr: u64) {
        threading.foreach_lapic(|lapic| {
            if lapic.get_id() as u32 != LocalApic::from_local_core().get_id() {
                threading.send_user_ipi(lapic.get_id(), |packet| {
                    packet.status = crate::processes::IpiStatus::Pending;
                    packet.request_type = crate::processes::IpiRequestType::SyncVMem;
                    packet.request_data.vmem_sync = IpiVMemSync::new(memory_block, vaddr)
                });
                if !threading.await_ipi(lapic.get_id()) {
                    simple_kernel_panic("GFS/sync", "IPI Request got stuck\n");
                }
            }
        });
    }

    pub fn sync(&self, threading: &Multithreading) {
        static mut LAST_FILE_PAGE_SYNCHRONISED: u32 = 0;
        static mut LAST_DIRECTORIES_PAGE_SYNCHRONISED: u32 = 0;
        static mut LAST_HASHMAP_PAGE_SYNCHRONISED: u32 = 0;
        static mut LAST_STRING_PAGE_SYNCHRONISED: u32 = 0;

        let files_bytes = (self.files.len() * size_of::<GfsFile>()) as u32;
        let directories_bytes = (self.directories.len() * size_of::<GfsDirectory>()) as u32;
        let hashmaps_bytes = (self.hashmaps.len() * size_of::<GfsDirectoryHashmap>()) as u32;
        let string_bytes = unsafe { GFS_STRING_ALLOCATED_SIZE };

        let mut files_pages = files_bytes / 0x1000;
        if files_bytes % 0x1000 != 0 {
            files_pages += 1;
        }
        let mut directory_pages = directories_bytes / 0x1000;
        if directories_bytes % 0x1000 != 0 {
            directory_pages += 1;
        }
        let hashmap_pages = hashmaps_bytes / 0x1000;
        if hashmaps_bytes % 0x1000 != 0 {
            directory_pages += 1;
        }
        let mut string_pages = string_bytes / 0x1000;
        if string_bytes % 0x1000 != 0 {
            string_pages += 1;
        }

        let new_file_pages = files_pages - unsafe { LAST_FILE_PAGE_SYNCHRONISED };
        let new_directory_pages = directory_pages - unsafe { LAST_DIRECTORIES_PAGE_SYNCHRONISED };
        let new_hashmap_pages = hashmap_pages - unsafe { LAST_HASHMAP_PAGE_SYNCHRONISED };
        let new_string_pages = string_pages - unsafe { LAST_STRING_PAGE_SYNCHRONISED };

        let pager = ref_processor_mut().ref_mut_pager();

        if new_file_pages != 0 {
            let mut vaddr =
                GFS_FILES_FIXED_VADDR + unsafe { LAST_FILE_PAGE_SYNCHRONISED } as u64 * 0x1000;
            let mut vaddr_at_ipi = vaddr;
            let base = unsafe { LAST_FILE_PAGE_SYNCHRONISED };
            let mut memory_block = MemoryBlock::empty();
            memory_block.base = pager.get_physical(vaddr).unwrap();
            for _ in base..files_pages {
                let physical = pager.get_physical(vaddr).unwrap();
                if physical != memory_block.base + memory_block.length {
                    GeneralFileSystem::send_sync_ipi(threading, memory_block.clone(), vaddr_at_ipi);
                    vaddr_at_ipi = vaddr;
                    memory_block.base = physical;
                    memory_block.length = 0x1000;
                } else {
                    memory_block.length += 0x1000;
                }
                vaddr += 0x1000;
            }
            if memory_block.length != 0 {
                GeneralFileSystem::send_sync_ipi(threading, memory_block, vaddr_at_ipi);
            }
            unsafe { LAST_FILE_PAGE_SYNCHRONISED = files_pages };
        }
        if new_directory_pages != 0 {
            let mut vaddr = GFS_DIRECTORY_FIXED_VADDR
                + unsafe { LAST_DIRECTORIES_PAGE_SYNCHRONISED } as u64 * 0x1000;
            let mut vaddr_at_ipi = vaddr;
            let base = unsafe { LAST_DIRECTORIES_PAGE_SYNCHRONISED };
            let mut memory_block = MemoryBlock::empty();
            memory_block.base = pager.get_physical(vaddr).unwrap();
            for _ in base..files_pages {
                let physical = pager.get_physical(vaddr).unwrap();
                if physical != memory_block.base + memory_block.length {
                    GeneralFileSystem::send_sync_ipi(threading, memory_block.clone(), vaddr_at_ipi);
                    vaddr_at_ipi = vaddr;
                    memory_block.base = physical;
                    memory_block.length = 0x1000;
                } else {
                    memory_block.length += 0x1000;
                }
                vaddr += 0x1000;
            }
            if memory_block.length != 0 {
                GeneralFileSystem::send_sync_ipi(threading, memory_block, vaddr_at_ipi);
            }
            unsafe { LAST_DIRECTORIES_PAGE_SYNCHRONISED = directory_pages };
        }
        if new_hashmap_pages != 0 {
            let mut vaddr =
                GFS_HASHMAP_FIXED_VADDR + unsafe { LAST_HASHMAP_PAGE_SYNCHRONISED } as u64 * 0x1000;
            let mut vaddr_at_ipi = vaddr;

            let base = unsafe { LAST_HASHMAP_PAGE_SYNCHRONISED };
            let mut memory_block = MemoryBlock::empty();
            memory_block.base = pager.get_physical(vaddr).unwrap();
            for _ in base..files_pages {
                let physical = pager.get_physical(vaddr).unwrap();
                if physical != memory_block.base + memory_block.length {
                    GeneralFileSystem::send_sync_ipi(threading, memory_block.clone(), vaddr_at_ipi);
                    vaddr_at_ipi = vaddr;
                    memory_block.base = physical;
                    memory_block.length = 0x1000;
                } else {
                    memory_block.length += 0x1000;
                }
                vaddr += 0x1000;
            }
            if memory_block.length != 0 {
                GeneralFileSystem::send_sync_ipi(threading, memory_block, vaddr_at_ipi);
            }
            unsafe { LAST_HASHMAP_PAGE_SYNCHRONISED = hashmap_pages };
        }
        if new_string_pages != 0 {
            let mut vaddr =
                GFS_STRINGS_FIXED_VADDR + unsafe { LAST_STRING_PAGE_SYNCHRONISED } as u64 * 0x1000;
            let mut vaddr_at_ipi = vaddr;

            let base = unsafe { LAST_STRING_PAGE_SYNCHRONISED };
            let mut memory_block = MemoryBlock::empty();
            memory_block.base = pager.get_physical(vaddr).unwrap();
            for _ in base..files_pages {
                let physical = pager.get_physical(vaddr).unwrap();
                if physical != memory_block.base + memory_block.length {
                    GeneralFileSystem::send_sync_ipi(threading, memory_block.clone(), vaddr_at_ipi);
                    vaddr_at_ipi = vaddr;
                    memory_block.base = physical;
                    memory_block.length = 0x1000;
                } else {
                    memory_block.length += 0x1000;
                }
                vaddr += 0x1000;
            }
            if memory_block.length != 0 {
                GeneralFileSystem::send_sync_ipi(threading, memory_block, vaddr_at_ipi);
            }
            unsafe { LAST_STRING_PAGE_SYNCHRONISED = string_pages };
        }
    }

    pub fn new(
        nvme_controller: Option<&NVMeController>,
        sata_controller: Option<&SataController>,
        allocator: &mut Allocator,
    ) -> Self {
        let mut module = Module::new("GFS");

        let file_system_array = scan_disks(allocator, nvme_controller, sata_controller);

        let mut _encountered_ext2_index = 0;

        let mut gfsa_index = 0;
        let mut gfsd_offset = 0;

        let gfs_array = allocator.alloc(1).unwrap();
        let gfs_data = allocator.alloc(1).unwrap();
        let ntfs_info = allocator.alloc(1).unwrap();
        let ntfs_roots: &'a mut [Option<GfsNtfsRoot>] =
            unsafe { slice::from_raw_parts_mut(ntfs_info.as_mut_ptr(), 26) };
        for i in 0..ntfs_roots.len() {
            let root: &mut Option<GfsNtfsRoot> = &mut ntfs_roots[i];
            *root = Option::None;
        }
        let file_systems: &'a mut [GfsFs<'a>] = unsafe {
            slice::from_raw_parts_mut(
                gfs_array.as_mut_ptr() as *mut GfsFs,
                file_system_array.num_total as usize,
            )
        };
        /* Loads and constructs basic structures of ext2 in memory*/
        /* TODO: Refactor this!*/
        for ext2_part in file_system_array.ext2 {
            let fs = &mut file_systems[gfsa_index];
            gfsd_offset = GeneralFileSystem::parse_ext2(
                ext2_part,
                &gfs_data,
                gfsd_offset,
                nvme_controller,
                sata_controller,
                fs,
                allocator,
            );
            gfsa_index += 1;
        }
        let fs_specific_data = gfs_data.base as *mut c_void;
        let detected_file_systems =
            unsafe { slice::from_raw_parts_mut(gfs_array.as_mut_ptr(), gfsa_index) };

        let linux_directory_index =
            GeneralFileSystem::assign_ntfs_roots(file_systems, ntfs_roots, &mut module);

        let smm_fs = GeneralFileSystem::find_smp(
            unsafe {
                slice::from_raw_parts_mut(
                    file_systems.as_ptr() as u64 as *mut GfsFs,
                    file_systems.len(),
                )
            },
            allocator,
        );

        let linux_root = GfsLinuxRoot {
            fs_index: smm_fs,
            directory_index: linux_directory_index,
        };

        /* 128 kb each*/
        /* actual allocated memory for the allocators*/
        let directory_allocator_memory_block = allocator.alloc_zero(32).unwrap();
        let file_allocator_memory_block = allocator.alloc_zero(32).unwrap();
        let hashmap_allocator_memory_block = allocator.alloc_zero(32).unwrap();
        let string_allocator_memory_block = allocator.alloc_zero(32).unwrap();

        GeneralFileSystem::map_virtual(
            &directory_allocator_memory_block,
            &file_allocator_memory_block,
            &hashmap_allocator_memory_block,
            &string_allocator_memory_block,
            allocator,
        );

        let directories = unsafe {
            slice::from_raw_parts_mut(
                GFS_DIRECTORY_FIXED_VADDR as *mut GfsDirectory,
                0x20000 / size_of::<GfsDirectory>(),
            )
        };

        let files = unsafe {
            slice::from_raw_parts_mut(
                GFS_FILES_FIXED_VADDR as *mut GfsFile,
                0x20000 / size_of::<GfsFile>(),
            )
        };
        let hashmaps = unsafe {
            slice::from_raw_parts_mut(
                GFS_HASHMAP_FIXED_VADDR as *mut GfsDirectoryHashmap,
                0x20000 / size_of::<GfsDirectoryHashmap>(),
            )
        };

        let mut ret = Self {
            detected_file_systems,
            fs_specific_data,
            ntfs_roots,
            linux_root,
            memory: [gfs_array, gfs_data, ntfs_info],
            directories,
            files,
            hashmaps,
            hashmap_manager: GfsDirectoryHashmapManager::new(allocator),
            files_allocated: 0,
            directories_allocated: file_system_array.num_total as u32 + 1,
            hashmaps_allocated: 0,
        };
        allocator.free(&file_system_array.file_system_data).unwrap();
        ret.create_partition_directories(allocator);
        success!(&mut module, "Initialized\n");
        return ret;
    }

    fn map_virtual(
        directory_allocator_memory_block: &MemoryBlock,
        file_allocator_memory_block: &MemoryBlock,
        hashmap_allocator_memory_block: &MemoryBlock,
        string_allocator_memory_block: &MemoryBlock,
        allocator: &mut Allocator,
    ) {
        let pager = unsafe {
            (*(FIXED_PROCESSOR_VIRTUAL_ADDRESS as *mut Processor))
                .pager
                .as_mut()
                .unwrap()
        };

        for i in 0..128 / 4 {
            pager
                .page_4_kb(
                    GFS_DIRECTORY_FIXED_VADDR + 0x1000 * i,
                    directory_allocator_memory_block.base + 0x1000 * i,
                    PAGER_RW | PAGER_PRESENT,
                    allocator,
                )
                .unwrap();
            pager
                .page_4_kb(
                    GFS_FILES_FIXED_VADDR + 0x1000 * i,
                    file_allocator_memory_block.base + 0x1000 * i,
                    PAGER_RW | PAGER_PRESENT,
                    allocator,
                )
                .unwrap();
            pager
                .page_4_kb(
                    GFS_HASHMAP_FIXED_VADDR + 0x1000 * i,
                    hashmap_allocator_memory_block.base + 0x1000 * i,
                    PAGER_RW | PAGER_PRESENT,
                    allocator,
                )
                .unwrap();
            pager
                .page_4_kb(
                    GFS_STRINGS_FIXED_VADDR + 0x1000 * i,
                    string_allocator_memory_block.base + 0x1000 * i,
                    PAGER_RW | PAGER_PRESENT,
                    allocator,
                )
                .unwrap();
        }
    }

    /* translates each partition into ntfs roots.
     * each ntfs root has a seperate directory entry
     * Why?
     *  |-> the system management partition 'smp' will corrospond to one linux partition.
     *      the linux partition will then be set as re  adonly, while the 'smp' partition will be read-write
     *      the flags corrosponding directory entry of the linux partition will be altered to readonly
     */
    fn assign_ntfs_roots(
        file_systems: &'a [GfsFs<'a>],
        ntfs_roots: &mut [Option<GfsNtfsRoot<'a>>],
        module: &mut Module,
    ) -> u32 {
        let mut linux_letter_index = 23usize;
        let mut linux_letter: &c_uchar = &NTFS_ROOT_NAMES[linux_letter_index];

        let mut _encountered_ext2_index = 0;

        let mut ntfs_root_index = 0;
        for i in 0..file_systems.len() {
            let fs = &file_systems[i];
            match &fs.fs_type {
                GfsFsType::Ext2 {
                    superblock: _,
                    extended_superblock: _,
                    physical: _,
                } => {
                    let ntfs_root: GfsNtfsRoot<'_> = GfsNtfsRoot {
                        file_system: fs,
                        directory_index: ntfs_root_index,
                        letter: *linux_letter,
                    };
                    ntfs_roots[linux_letter_index + 2] = Option::Some(ntfs_root);

                    info!(
                        module,
                        "Linux Partition mounted: {}\n", *linux_letter as char
                    );
                    if linux_letter_index == 0 {
                        simple_kernel_panic("GFS", "Detected more than 23 linux partitions\n")
                    }
                    linux_letter_index -= 1;
                    linux_letter = &NTFS_ROOT_NAMES[linux_letter_index];
                    ntfs_root_index += 1;
                    _encountered_ext2_index += 1;
                }
                GfsFsType::Nothing => {}
            }
        }
        return ntfs_root_index;
    }

    /* Info: Only linux partition can be smm partitions
     * locates the system management partition. ('smp' under '/mnt/' and '/', so the linux root)
     * Also make this a little bit more robust maybe
     */
    fn find_smp(file_systems: &mut [GfsFs], allocator: &mut Allocator) -> u32 {
        for i in 0..file_systems.len() {
            let fs = &file_systems[i];
            match &fs.fs_type {
                GfsFsType::Ext2 {
                    superblock: _,
                    extended_superblock: _,
                    physical,
                } => {
                    if physical.root_contains(allocator, fs.disk, "config.smcfg") {
                        return i as u32;
                    }
                }
                GfsFsType::Nothing => {}
            }
        }
        simple_kernel_panic("GFS", "Could not find system management partition\n")
    }

    fn parse_ext2(
        ext2_part: &Ext2Fs,
        gfs_data: &MemoryBlock,
        mut gfsd_offset: u64,
        nvme_controller: Option<&NVMeController>,
        sata_controller: Option<&SataController>,
        fs: &mut GfsFs,
        allocator: &mut Allocator,
    ) -> u64 {
        let superblock = (gfs_data.base + gfsd_offset) as *mut Ext2Superblock;
        gfsd_offset += size_of::<Ext2Superblock>() as u64;
        unsafe { *superblock = ext2_part.superblock }
        let extended_block;
        if let Some(extended) = ext2_part.extended_superblock {
            let extended_superblock =
                (gfs_data.base + gfsd_offset as u64) as *mut Ext2ExtendedSuperblock;
            gfsd_offset += size_of::<Ext2ExtendedSuperblock>() as u64;
            unsafe { *extended_superblock = extended }
            extended_block = Option::Some(unsafe { extended_superblock.as_ref().unwrap() });
        } else {
            extended_block = Option::None;
        }
        let physical_address = unsafe {
            ((gfs_data.base + gfsd_offset as u64) as *mut PhysicalExt2)
                .as_mut()
                .unwrap()
        };
        gfsd_offset += size_of::<PhysicalExt2>() as u64;
        let superblock_mut_ref = unsafe { superblock.as_mut().unwrap() };

        if let Some(cntrl) = nvme_controller {
            match cntrl.get_disk(ext2_part.disk_ident) {
                Some(disk) => fs.disk = disk,
                None => {
                    if let Some(sata) = sata_controller {
                        match sata.get_disk(ext2_part.disk_ident) {
                            Some(disk) => fs.disk = disk,
                            None => simple_kernel_panic("GFS", "Unkown Disk Identifier\n"),
                        }
                    } else {
                        simple_kernel_panic("GFS", "Unkown Disk Identifier\n")
                    }
                }
            };
        } else if let Some(sata) = sata_controller {
            match sata.get_disk(ext2_part.disk_ident) {
                Some(disk) => fs.disk = disk,
                None => simple_kernel_panic("GFS", "Unknown Disk Identifier\n"),
            }
        }
        fs.starting_lba = ext2_part.starting_lba;
        let total_number_blocks = superblock_mut_ref.total_number_blocks;
        fs.file_system_size = (1024 << superblock_mut_ref.block_size) * total_number_blocks as u64;
        *physical_address = GeneralFileSystem::mount_ext2(
            ext2_part.starting_lba,
            unsafe { superblock.as_ref().unwrap() },
            extended_block,
            allocator,
            fs.disk,
        );
        fs.fs_type = GfsFsType::Ext2 {
            superblock: superblock_mut_ref,
            extended_superblock: extended_block,
            physical: physical_address,
        };
        return gfsd_offset;
    }

    /* creates the mountpoints. 'mnt/x/', ...*/
    pub fn create_partition_directories(&mut self, allocator: &mut Allocator) {
        let fs: &GfsFs<'a> = &self.detected_file_systems[self.linux_root.fs_index as usize];
        match &fs.fs_type {
            GfsFsType::Ext2 {
                superblock: _,
                extended_superblock: _,
                physical,
            } => {
                if !physical.root_contains(allocator, fs.disk, "mnt") {
                    simple_kernel_panic("GFS", "Linux root does not contain mnt directory\n")
                }
            }
            GfsFsType::Nothing => {}
        }
        /* Assigns a hashmap to each root*/
        {
            /* Borrow checker is really annoying*/
            let scummy_hashmaps =
                slice_from_raw_parts_mut(self.hashmaps.as_mut_ptr(), self.hashmaps.len());
            unsafe {
                (*scummy_hashmaps)[self.hashmaps_allocated as usize] =
                    self.hashmap_manager.allocate_hashmap(16);

                self.hashmaps_allocated += 1;
                self.directories[self.linux_root.directory_index as usize] = GfsDirectory::new(
                    self.linux_root.fs_index,
                    2,
                    &mut (*scummy_hashmaps)[0],
                    GFS_PERMISSIONS_WRITE,
                );
                self.directories[self.linux_root.directory_index as usize].set_completed();
            }
            for i in 0..self.ntfs_roots.len() {
                let root = &self.ntfs_roots[i as usize];
                match root {
                    Some(fs_root) => match &fs_root.file_system.fs_type {
                        GfsFsType::Ext2 {
                            superblock: _,
                            extended_superblock: _,
                            physical,
                        } => {
                            let fs_index = unsafe {
                                (fs_root.file_system as *const GfsFs)
                                    .offset_from_unsigned(self.detected_file_systems.as_ptr())
                            };

                            let hashmap;
                            if fs_root.file_system as *const GfsFs == fs as *const GfsFs {
                                /* Linux Root*/
                                hashmap = unsafe { &mut (*scummy_hashmaps)[0] };
                            } else {
                                unsafe {
                                    (*scummy_hashmaps)[self.hashmaps_allocated as usize] =
                                        self.hashmap_manager.allocate_hashmap(16);
                                    hashmap =
                                        &mut (*scummy_hashmaps)[self.hashmaps_allocated as usize];
                                };
                                self.hashmaps_allocated += 1;
                            }
                            let mut directory = GfsDirectory::new(
                                fs_index as u32,
                                2, /* 2 is always root for ext2*/
                                hashmap,
                                GFS_PERMISSIONS_WRITE,
                            );
                            physical.load_root(
                                fs.disk,
                                &mut directory,
                                self,
                                allocator,
                                fs_index as u32,
                            );
                            directory.set_completed();
                            self.directories[fs_root.directory_index as usize] = directory;
                        }
                        GfsFsType::Nothing => {}
                    },
                    None => {}
                }
            }
        }

        let linux_root_directory = unsafe {
            &mut (*slice_from_raw_parts_mut(self.directories.as_mut_ptr(), self.directories.len()))
                [self.linux_root.directory_index as usize]
        };
        let mount_directory = linux_root_directory
            .open_directory("mnt", self, allocator)
            .unwrap();
        for i in 0..self.ntfs_roots.len() {
            let raw_ntfs_root = &self.ntfs_roots[i as usize];
            if let Option::Some(ntfs_root) = raw_ntfs_root {
                let name = unsafe {
                    str::from_utf8_unchecked(slice::from_raw_parts(
                        &ntfs_root.letter as *const u8,
                        1,
                    ))
                };
                if let GfsInsertion::AllreadyPresent =
                    mount_directory.add_virtual_directory(name, ntfs_root.directory_index, self)
                {
                    simple_kernel_panic("GFS", "mnt directory must be empty\n")
                }
            }
        }
        if let GfsInsertion::AllreadyPresent =
            mount_directory.add_virtual_directory("smp", self.linux_root.directory_index, self)
        {
            simple_kernel_panic("GFS", "mnt directory must be empty\n")
        }
    }

    pub fn mount_ext2(
        starting_lba: u64,
        superblock: &'a Ext2Superblock,
        extended_superblock: Option<&Ext2ExtendedSuperblock>,
        allocator: &mut Allocator,
        disk: &dyn Disk,
    ) -> PhysicalExt2<'a> {
        let ret = PhysicalExt2::load(
            superblock,
            extended_superblock,
            disk,
            allocator,
            starting_lba,
        );
        return ret;
    }

    pub fn linux_root_directory_index(&self) -> u32 {
        return self.linux_root.directory_index;
    }

    pub fn linux_root_fs_index(&self) -> u32 {
        return self.linux_root.fs_index;
    }

    pub fn request_hashmap(&mut self, num_entries: u32) -> (u32, *mut GfsDirectoryHashmap) {
        let index = self.hashmaps_allocated;
        let length = self.hashmaps.len() as u32;
        let ret = &mut self.hashmaps[index as usize] as *mut GfsDirectoryHashmap;
        unsafe { *&mut *ret = self.hashmap_manager.allocate_hashmap(num_entries) };
        if self.hashmaps_allocated + 1 > length {
            todo!("expand hashmaps");
        }
        self.hashmaps_allocated += 1;
        return (index, ret);
    }

    pub fn request_file(&mut self) -> (u32, *mut GfsFile) {
        let index = self.files_allocated;
        let length = self.files.len() as u32;
        let ret = &mut self.files[index as usize] as *mut GfsFile;
        if self.files_allocated + 1 > length {
            todo!("expand files");
        }
        self.files_allocated += 1;
        return (index, ret);
    }

    pub fn request_directory(&mut self) -> (u32, *mut GfsDirectory<'static>) {
        let index = self.directories_allocated;
        let length = self.directories.len() as u32;
        let ret = unsafe {
            &mut (*slice_from_raw_parts_mut(self.directories.as_mut_ptr(), self.directories.len()))
                [index as usize]
        } as *mut GfsDirectory;
        if self.directories_allocated + 1 > length {
            todo!("expand files");
        }
        self.directories_allocated += 1;
        return (index, ret);
    }

    pub fn ref_directory(&mut self, directory: u32) -> *mut GfsDirectory<'static> {
        &self.directories[directory as usize] as *const GfsDirectory as u64 as *mut GfsDirectory
    }

    pub fn ref_file(&self, file: u32) -> &'static GfsFile {
        return unsafe {
            (&self.files[file as usize] as *const GfsFile)
                .as_ref()
                .unwrap()
        };
    }

    pub fn ref_file_mut(&self, file: u32) -> *mut GfsFile {
        return &self.files[file as usize] as *const GfsFile as *mut GfsFile;
    }

    pub fn get_manager(&mut self) -> *mut GfsDirectoryHashmapManager<'a> {
        return &self.hashmap_manager as *const GfsDirectoryHashmapManager as u64
            as *mut GfsDirectoryHashmapManager;
    }

    pub fn align_size_to_blocks(&self, size: u64, fs_index: u32) -> u64 {
        let fs = &self.detected_file_systems[fs_index as usize];
        match &fs.fs_type {
            Ext2 {
                superblock,
                extended_superblock: _,
                physical: _,
            } => {
                let block_size_in_bytes = 1024 << superblock.block_size;
                let mut new_size = (size / block_size_in_bytes) * block_size_in_bytes;
                if new_size != size {
                    new_size += block_size_in_bytes;
                }
                return new_size;
            }
            GfsFsType::Nothing => return 0,
        }
    }
}

#[derive(Debug)]
pub enum GfsReadError {
    Sucess { bytes_read: u64 },
    BufferToSmall,
    DiskError(DiskIOResult),
}
pub enum GfsResult {
    InvalidLength,
    Malformed,
    NotFound,
    DiscoveryPossible,
    MismatchedType,
    ReadError(GfsReadError),
}

impl<'a> GeneralFileSystem<'a> {
    /** requires that path is without the drive letter*/
    fn win_get_link(&self, root: &GfsNtfsRoot, path: &str) -> Result<GfsLink, GfsResult> {
        let mut directory: (GfsLink, &GfsDirectory) = (
            GfsLink::new(root.directory_index, GfsType::Directory),
            unsafe {
                &*(GFS_DIRECTORY_FIXED_VADDR as *mut GfsDirectory)
                    .add(root.directory_index as usize)
            },
        );
        let mut iterator = path.split_terminator('/').peekable();
        while let Option::Some(to_lookup) = iterator.next() {
            let dir = directory.1;

            match dir.get_entries().lookup(&self.hashmap_manager, to_lookup) {
                Some(link) => match link.link_type() {
                    GfsType::Directory => {
                        directory = (link, unsafe {
                            &*(GFS_DIRECTORY_FIXED_VADDR as *const GfsDirectory)
                                .add(link.index() as usize)
                        });
                    }
                    GfsType::File => {
                        if !iterator.peek().is_none() {
                            /* Not the last*/
                            return Result::Err(GfsResult::Malformed);
                        } else {
                            return Result::Ok(link);
                        }
                    }
                },
                None => {
                    if dir.is_completed() {
                        return Result::Err(GfsResult::DiscoveryPossible);
                    } else {
                        return Result::Err(GfsResult::NotFound);
                    }
                }
            }
        }
        return Result::Ok(directory.0);
    }

    fn win_get_file_mut(&self, path: &str) -> Result<&'a mut GfsFile, GfsResult> {
        if path.len() > 0x1000 || path.len() == 0 {
            return Result::Err(GfsResult::InvalidLength);
        }
        let root_index = to_lowercase(path.as_bytes()[0] as char) as u8 - 'a' as u8;
        match &self.ntfs_roots[root_index as usize] {
            Some(root) => {
                if path.as_bytes()[1] != ':' as u8 || path.as_bytes()[2] != '\\' as u8 {
                    return Result::Err(GfsResult::Malformed);
                }
                match self.win_get_link(root, &path[3..]) {
                    Ok(link) => {
                        if let GfsType::File = link.link_type() {
                            return Result::Ok(unsafe {
                                &mut *(GFS_FILES_FIXED_VADDR as *mut GfsFile)
                                    .add(link.index() as usize)
                            });
                        } else {
                            return Result::Err(GfsResult::MismatchedType);
                        }
                    }
                    Err(e) => return Result::Err(e),
                }
            }

            None => return Result::Err(GfsResult::NotFound),
        }
    }

    fn lin_get_link(&self, path: &str) -> Result<GfsLink, GfsResult> {
        let mut directory: (GfsLink, &GfsDirectory) = (
            GfsLink::new(self.linux_root.directory_index, GfsType::Directory),
            unsafe {
                &*(GFS_DIRECTORY_FIXED_VADDR as *mut GfsDirectory)
                    .add(self.linux_root.directory_index as usize)
            },
        );
        let mut iterator = path.split_terminator('/').peekable();
        while let Option::Some(to_lookup) = iterator.next() {
            let dir = directory.1;

            match dir.get_entries().lookup(&self.hashmap_manager, to_lookup) {
                Some(link) => match link.link_type() {
                    GfsType::Directory => {
                        directory = (link, unsafe {
                            &*(GFS_DIRECTORY_FIXED_VADDR as *const GfsDirectory)
                                .add(link.index() as usize)
                        });
                    }
                    GfsType::File => {
                        if !iterator.peek().is_none() {
                            /* Not the last*/
                            return Result::Err(GfsResult::Malformed);
                        } else {
                            return Result::Ok(link);
                        }
                    }
                },
                None => {
                    if dir.is_completed() {
                        return Result::Err(GfsResult::DiscoveryPossible);
                    } else {
                        return Result::Err(GfsResult::NotFound);
                    }
                }
            }
        }
        return Result::Ok(directory.0);
    }

    /* without the first '/'*/
    fn lin_get_directory(&self, path: &str) -> Result<&'a GfsDirectory, GfsResult> {
        match self.lin_get_link(path) {
            Ok(link) => {
                if let GfsType::Directory = link.link_type() {
                    return Result::Ok(unsafe {
                        &*((GFS_DIRECTORY_FIXED_VADDR as *const GfsDirectory)
                            .add(link.index() as usize))
                    });
                } else {
                    return Result::Err(GfsResult::MismatchedType);
                }
            }
            Err(e) => return Result::Err(e),
        }
    }

    pub fn get_file_mut(&self, path: &str) -> Result<&'a mut GfsFile, GfsResult> {
        if path.as_bytes()[0] == '/' as u8 {
            todo!("Implement linux path");
        }
        return self.win_get_file_mut(path);
    }

    pub fn get_directory(&self, path: &str) -> Result<&'a GfsDirectory, GfsResult> {
        if path.as_bytes()[0] == '/' as u8 {
            return self
                .lin_get_directory(unsafe { str::from_utf8_unchecked(&path.as_bytes()[1..]) });
        } else {
            todo!("Implement windows directory");
        }
    }

    pub fn lin_discover_directory(
        &mut self,
        physical_allocator: &mut Allocator,
        path: &str,
        scheduler: u8,
    ) -> Option<GfsResult> {
        let mut directory: (GfsLink, &GfsDirectory) = (
            GfsLink::new(self.linux_root.directory_index, GfsType::Directory),
            unsafe {
                &*(GFS_DIRECTORY_FIXED_VADDR as *mut GfsDirectory)
                    .add(self.linux_root.directory_index as usize)
            },
        );
        let mut iterator = path.split_terminator('/').peekable();
        while let Option::Some(to_lookup) = iterator.next() {
            let dir = directory.1;
            match dir.get_entries().lookup(&self.hashmap_manager, to_lookup) {
                Some(link) => {
                    if let GfsType::Directory = link.link_type() {
                        let directory_to_discover = unsafe {
                            &mut *(GFS_DIRECTORY_FIXED_VADDR as *mut GfsDirectory)
                                .add(link.index() as usize)
                        };
                        if !directory_to_discover.is_completed() {
                            let fs_index = directory_to_discover.get_fs_index();
                            let phys_storage_node =
                                directory_to_discover.get_physical_storage_node();

                            let fs = &self.detected_file_systems[fs_index as usize];
                            match &fs.fs_type {
                                GfsFsType::Ext2 {
                                    superblock: _,
                                    extended_superblock: _,
                                    physical,
                                } => {
                                    match physical.discover_directory(
                                        fs_index,
                                        self,
                                        directory_to_discover,
                                        fs.disk,
                                        physical_allocator,
                                        phys_storage_node,
                                        scheduler,
                                    ) {
                                        Some(e) => return Option::Some(GfsResult::ReadError(e)),
                                        None => {}
                                    }
                                }
                                GfsFsType::Nothing => {}
                            }
                        }
                        directory_to_discover.set_completed();
                        directory.0 = link;
                        directory.1 = directory_to_discover;
                    } else {
                        return Option::Some(GfsResult::MismatchedType);
                    }
                }
                None => {
                    /* the first Directory (linux root) is allways completed*/
                    return Option::Some(GfsResult::NotFound);
                }
            }
        }
        return Option::None;
    }
    pub fn discover_directory(
        &mut self,
        path: &str,
        physical_allocator: &mut Allocator,
        scheduler: u8,
    ) -> Option<GfsResult> {
        if path.as_bytes()[0] == '/' as u8 {
            return self.lin_discover_directory(
                physical_allocator,
                unsafe { str::from_utf8_unchecked(&path.as_bytes()[1..]) },
                scheduler,
            );
        } else {
            todo!("Implement windows directory discovery")
        }
    }

    pub fn read_pointers_into_buffer(
        &self,
        buffer: &Buffer,
        pointers: &[u32],
        fs_index: u32,
        scheduler: u8,
    ) -> Result<u64, GfsReadError> {
        let fs = &self.detected_file_systems[fs_index as usize];

        match &fs.fs_type {
            GfsFsType::Ext2 {
                superblock: _,
                extended_superblock: _,
                physical,
            } => {
                return physical
                    .read_pointer_content_into_buffer(buffer, pointers, fs.disk, scheduler);
            }
            GfsFsType::Nothing => {
                return Result::Ok(0);
            }
        }
    }

    pub fn block_size(&self, fs_index: u32) -> u32 {
        match &self.detected_file_systems[fs_index as usize].fs_type {
            GfsFsType::Ext2 {
                superblock,
                extended_superblock: _,
                physical: _,
            } => {
                return 1024 << superblock.block_size;
            }
            GfsFsType::Nothing => return 0,
        }
    }
    /* second return is the number of block pointers present*/

    pub fn read_singly_indirect_pointer_block(
        &self,
        buffer: &Buffer,
        allocator: &mut Allocator,
        singly_indirect_pointer: u32,
        fs_index: u32,
        scheduler: u8,
    ) -> Result<u64, GfsReadError> {
        let fs = &self.detected_file_systems[fs_index as usize];
        match &fs.fs_type {
            GfsFsType::Ext2 {
                superblock: _,
                extended_superblock: _,
                physical,
            } => {
                return physical.read_singly_indirect_pointer_block(
                    buffer,
                    allocator,
                    singly_indirect_pointer,
                    fs.disk,
                    scheduler,
                );
            }
            GfsFsType::Nothing => return Result::Ok(0),
        }
    }
}
