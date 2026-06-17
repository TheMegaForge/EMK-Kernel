use crate::{
    hal::memory::allocator::{Allocator, VirtualAllocator},
    utils::buffer::Buffer,
    vfs::gfs::{
        GFS_FLAG_CLOSED, GFS_PERMISSIONS_SYSTEM_ACCESS, GFS_PERMISSIONS_WRITE,
        GFS_PERMISSIONS_WRITE_SHARING, GeneralFileSystem, GfsPermissions,
    },
};

pub type UniqueFileIdentifer = u32;

pub struct GfsFile {
    fs_index: u32,
    physical_storage_node: u32,
    direct_data_pointers: [u32; 12],
    singly_indirect_data_pointer: u32,
    doubly_indirect_data_pointer: u32,
    triply_indirect_data_pointer: u32,
    permissions: GfsPermissions,
    read_count: u16,
    write_count: u16,
    flags: u16,
    id: UniqueFileIdentifer,
    reserved: u32,
    size: u64,
    virtualization: u64,
}

static mut CURRENT_UNIQUE_FILE_IDENTIFIER: UniqueFileIdentifer = 1;

impl GfsFile {
    pub fn get_permissions(&self) -> GfsPermissions {
        return self.permissions;
    }

    pub fn remove_write_permission(&mut self) {
        if self.permissions & GFS_PERMISSIONS_WRITE != 0 {
            self.permissions ^= GFS_PERMISSIONS_WRITE;
        }
        if self.permissions & GFS_PERMISSIONS_WRITE_SHARING != 0 {
            self.permissions ^= GFS_PERMISSIONS_WRITE_SHARING;
        }
    }

    pub fn set_system_access(&mut self) {
        self.permissions |= GFS_PERMISSIONS_SYSTEM_ACCESS;
    }

    #[inline(always)]
    pub fn increase_read_count(&mut self) {
        self.read_count += 1;
    }

    #[inline(always)]
    pub fn increase_write_count(&mut self) {
        self.write_count += 1;
    }

    #[inline(always)]
    pub fn decrease_read_count(&mut self) {
        self.read_count -= 1;
    }

    #[inline(always)]
    pub fn decrease_write_count(&mut self) {
        self.write_count -= 1;
    }

    #[inline(always)]
    pub fn get_read_count(&self) -> u16 {
        return self.read_count;
    }
    #[inline(always)]
    pub fn get_write_count(&self) -> u16 {
        return self.write_count;
    }
    /**
     * bool 1 => must not deallocate
     * bool 0 => must deallocate
     */
    pub fn read(
        &self,
        physical_allocator: &mut Allocator,
        gfs: &GeneralFileSystem,
        scheduler: u8,
    ) -> (Buffer, bool) {
        if self.virtualization != 0 {
            let mut size = self.size;

            if size % 0x1000 != 0 {
                size += 0x1000 - size % 0x1000;
            }

            return (
                Buffer::from_existing(self.virtualization, size, self.size, true),
                true,
            );
        }
        let buffer = Buffer::new(
            physical_allocator,
            false,
            gfs.align_size_to_blocks(self.size, self.fs_index),
        );
        let mut buffer_offset = gfs
            .read_pointers_into_buffer(
                &buffer,
                &self.direct_data_pointers,
                self.fs_index,
                scheduler,
            )
            .unwrap();

        let max_bytes_for_direct_pointers = 12 * gfs.block_size(self.fs_index);
        if max_bytes_for_direct_pointers as u64 >= self.size {
            /* No Singly, Doubly or Triply indirect pointers*/
            return (buffer, false);
        }
        let max_bytes_for_singly_indirect_pointers = (gfs.block_size(self.fs_index) as u64
            / size_of::<u32>() as u64)
            * gfs.block_size(self.fs_index) as u64;

        buffer_offset = gfs
            .read_singly_indirect_pointer_block(
                &buffer
                    .sub_buffer(buffer_offset, buffer.get_size() - buffer_offset)
                    .unwrap(),
                physical_allocator,
                self.singly_indirect_data_pointer,
                self.fs_index,
                scheduler,
            )
            .unwrap();

        if max_bytes_for_singly_indirect_pointers > self.size - max_bytes_for_direct_pointers as u64
        {
            /* No Doubly or Triply indirect pointers*/
            return (buffer, false);
        }
        todo!("Implement doubly/triply indirect pointers")
    }
    /**
     *  returned buffer has the virtual address,
     *  but under the hood the physical address is used
     *  bool true => memory must not be deallocated
     *  bool false => memory must be deallocated
     */
    pub fn read_virtual(
        &self,
        physical_allocator: &mut Allocator,
        virtual_allocator: &mut VirtualAllocator,
        gfs: &GeneralFileSystem,
        scheduler: u8,
    ) -> (Buffer, bool) {
        if self.virtualization != 0 {
            let mut size = self.size;
            if size % 0x1000 != 0 {
                size += 0x1000 - size % 0x1000;
            }

            return (
                Buffer::from_existing(self.virtualization, self.size, self.size, true),
                true,
            );
        }

        let (buffer, mb) = Buffer::new_physical_virtual(
            virtual_allocator,
            false,
            gfs.align_size_to_blocks(self.size, self.fs_index),
        );

        let mut buffer_offset = gfs
            .read_pointers_into_buffer(
                &buffer,
                &self.direct_data_pointers,
                self.fs_index,
                scheduler,
            )
            .unwrap();

        let max_bytes_for_direct_pointers = 12 * gfs.block_size(self.fs_index);
        if max_bytes_for_direct_pointers as u64 >= self.size {
            /* No Singly, Doubly or Triply indirect pointers*/
            return (
                Buffer::from_existing(mb.base, mb.length, self.size, true),
                false,
            );
        }
        let max_bytes_for_singly_indirect_pointers = (gfs.block_size(self.fs_index) as u64
            / size_of::<u32>() as u64)
            * gfs.block_size(self.fs_index) as u64;

        buffer_offset = gfs
            .read_singly_indirect_pointer_block(
                &buffer
                    .sub_buffer(buffer_offset, buffer.get_size() - buffer_offset)
                    .unwrap(),
                physical_allocator,
                self.singly_indirect_data_pointer,
                self.fs_index,
                scheduler,
            )
            .unwrap();

        if max_bytes_for_singly_indirect_pointers > self.size - max_bytes_for_direct_pointers as u64
        {
            /* No Doubly or Triply indirect pointers*/
            return (
                Buffer::from_existing(mb.base, mb.length, self.size, true),
                false,
            );
        }
        todo!("Implement doubly/triply indirect pointers")
    }

    pub fn virtualize_virtual(
        &mut self,
        gfs: &GeneralFileSystem,
        physical_allocator: &mut Allocator,
        virtual_allocator: &mut VirtualAllocator,
        scheduler: u8,
    ) -> bool {
        if self.virtualization != 0 {
            return false;
        }
        self.virtualization = self
            .read_virtual(physical_allocator, virtual_allocator, gfs, scheduler)
            .0
            .address();

        return true;
    }

    pub fn new(
        fs_index: u32,
        physical_storage_node: u32,
        direct_data_pointers: [u32; 12],
        singly_indirect_data_pointer: u32,
        doubly_indirect_data_pointer: u32,
        triply_indirect_data_pointer: u32,
        size: u64,
        permissions: GfsPermissions,
    ) -> Self {
        let id = unsafe { CURRENT_UNIQUE_FILE_IDENTIFIER };
        unsafe {
            CURRENT_UNIQUE_FILE_IDENTIFIER += 1;
        }
        return Self {
            fs_index,
            physical_storage_node,
            direct_data_pointers,
            singly_indirect_data_pointer,
            doubly_indirect_data_pointer,
            triply_indirect_data_pointer,
            permissions,
            size,
            id,
            write_count: 0,
            read_count: 0,
            flags: GFS_FLAG_CLOSED,
            reserved: 0,
            virtualization: 0,
        };
    }
}
