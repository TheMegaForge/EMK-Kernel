use crate::{
    hal::memory::allocator::Allocator,
    vfs::gfs::{
        GeneralFileSystem, GfsPermissions, GfsType,
        directory_hashmap::{
            GfsDirectoryHashmap, GfsDirectoryHashmapInsertion, GfsDirectoryHashmapManager,
        },
        file::GfsFile,
        link::GfsLink,
    },
};

const GFS_DIRECTORY_COMPLETED_PHYSICAL_LOOKUP: u8 = 1;

pub struct GfsDirectory<'a> {
    fs_index: u32,
    physical_storage_node: u32,
    entries: &'a mut GfsDirectoryHashmap,
    permissions: GfsPermissions,
    flags: u8,
}

pub enum GfsInsertion {
    AllreadyPresent,
    Success,
    TechnicalFailure,
}

impl<'a> GfsDirectory<'a> {
    pub fn new(
        fs_index: u32,
        physical_storage_node: u32,
        entries: &'a mut GfsDirectoryHashmap,
        permissions: GfsPermissions,
    ) -> Self {
        return Self {
            fs_index,
            physical_storage_node,
            entries,
            permissions: permissions,
            flags: 0,
        };
    }

    pub fn get_fs_index(&self) -> u32 {
        return self.fs_index;
    }
    pub fn get_physical_storage_node(&self) -> u32 {
        return self.physical_storage_node;
    }

    pub fn get_entries_mut(&mut self) -> &'a mut GfsDirectoryHashmap {
        unsafe {
            (self.entries as *const GfsDirectoryHashmap as u64 as *mut GfsDirectoryHashmap)
                .as_mut()
                .unwrap()
        }
    }

    pub fn get_entries(&self) -> &'a GfsDirectoryHashmap {
        unsafe { &*(self.entries as *const GfsDirectoryHashmap) }
    }

    pub(in crate::vfs::gfs) fn set_completed(&mut self) {
        self.flags |= GFS_DIRECTORY_COMPLETED_PHYSICAL_LOOKUP;
    }

    pub fn new_link(
        &mut self,
        name: &str,
        link: GfsLink,
        manager: &mut GfsDirectoryHashmapManager,
    ) -> bool {
        if let GfsDirectoryHashmapInsertion::Success { rehashed: _ } =
            self.entries.insert(manager, name, link)
        {
            return true;
        } else {
            return false;
        }
    }

    pub fn add_virtual_directory(
        &mut self,
        name: &str,
        link_destination: u32,
        gfs: &mut GeneralFileSystem,
    ) -> GfsInsertion {
        return match self.entries.insert(
            unsafe { &mut *gfs.get_manager() },
            name,
            GfsLink::new(link_destination, GfsType::Directory),
        ) {
            GfsDirectoryHashmapInsertion::AllreadyPresent => GfsInsertion::AllreadyPresent,
            GfsDirectoryHashmapInsertion::Success { rehashed: _ } => GfsInsertion::Success,
            _ => GfsInsertion::TechnicalFailure,
        };
    }

    pub fn open_directory(
        &mut self,
        str: &str,
        gfs: &mut GeneralFileSystem,
        allocator: &mut Allocator,
    ) -> Option<&'static mut GfsDirectory<'static>> {
        match self.entries.lookup(&mut gfs.hashmap_manager, str) {
            Some(link) => {
                if let GfsType::Directory = link.link_type() {
                    return Option::Some(unsafe { &mut *gfs.ref_directory(link.index()) });
                } else {
                    return Option::None;
                }
            }
            None => {
                if self.flags & 1 == 1 {
                    return None;
                }
                let fs = &gfs.detected_file_systems[self.fs_index as usize];
                let mut ret = Option::None;
                match &fs.fs_type {
                    crate::vfs::gfs::GfsFsType::Ext2 {
                        superblock,
                        extended_superblock: _,
                        physical,
                    } => {
                        let physical_directory = physical.directory_from_inode(
                            allocator,
                            self.physical_storage_node,
                            fs.disk,
                        );
                        let second_allocator =
                            unsafe { (allocator as *mut Allocator).as_mut().unwrap() };
                        physical_directory.foreach_entry(
                            second_allocator,
                            fs.disk,
                            superblock.block_size as u16,
                            fs.starting_lba,
                            |entry| match entry.type_indicator {
                                1 => {
                                    let (link_index, file) = gfs.request_file();
                                    let inode =
                                        physical.read_inode(entry.inode, fs.disk, allocator);
                                    unsafe {
                                        *file = GfsFile::new(
                                            self.fs_index,
                                            entry.inode,
                                            inode.direct_block_pointers,
                                            inode.singly_indirect_block_pointer,
                                            inode.doubly_indirect_block_pointer,
                                            inode.triply_indirect_block_pointer,
                                            inode.low_size as u64,
                                            self.permissions,
                                        )
                                    }
                                    self.new_link(
                                        entry.string,
                                        GfsLink::new(link_index, GfsType::File),
                                        &mut gfs.hashmap_manager,
                                    );
                                }
                                2 => {
                                    let (link_index, directory) = gfs.request_directory();
                                    let (_, hashmap) = gfs.request_hashmap(32);
                                    unsafe {
                                        *directory = GfsDirectory::new(
                                            self.fs_index,
                                            entry.inode,
                                            hashmap.as_mut().unwrap(),
                                            self.permissions,
                                        )
                                    };

                                    if entry.string == str {
                                        ret = Option::Some(unsafe { directory.as_mut().unwrap() });
                                    }

                                    self.new_link(
                                        entry.string,
                                        GfsLink::new(link_index, GfsType::Directory),
                                        &mut gfs.hashmap_manager,
                                    );
                                }

                                _ => todo!(),
                            },
                        );
                    }
                    crate::vfs::gfs::GfsFsType::Nothing => {}
                }
                self.flags |= GFS_DIRECTORY_COMPLETED_PHYSICAL_LOOKUP;
                return ret;
            }
        }
    }
    #[inline(always)]
    pub fn is_completed(&self) -> bool {
        return self.flags & GFS_DIRECTORY_COMPLETED_PHYSICAL_LOOKUP != 0;
    }
}
