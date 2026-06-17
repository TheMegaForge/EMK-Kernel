use core::ffi::c_uchar;

use crate::{
    aml::NameString,
    hal::memory::allocator::Allocator,
    utils::{allocators::PageAllocator, stack::Stack, string::String},
};
#[derive(Clone, Copy)]
pub struct Frame {
    /*
     * true => append() called
     * false => push() called
     * Option::None => neither append() or push() called
     */
    appended: Option<bool>,

    /*
     * push_frame() called.
     */
    extended: bool,
}

impl Default for Frame {
    fn default() -> Self {
        return Self {
            appended: Option::None,
            extended: false,
        };
    }
}

pub struct PathSystem {
    storage: String,
    frame_stack: Stack<Frame>,
    /*
     * [u16, u8, u8] = (offset into storage, length in segments, owner root)
     */
    roots: PageAllocator<(u16, u8, u16)>, //TODO: this!
    active_paths: Stack<u16>,
}

impl Default for PathSystem {
    fn default() -> Self {
        return Self {
            storage: String::default(),
            frame_stack: Stack::<Frame>::default(),
            roots: PageAllocator::default(),
            active_paths: Stack::<u16>::default(),
        };
    }
}

impl PathSystem {
    //TODO: this!
    pub fn new(allocator: &mut Allocator) -> Self {
        let mut storage = String::new(allocator, 0x4000);
        let frame_stack = Stack::new(allocator, 256);
        let mut roots = PageAllocator::new(allocator, 0x1000);
        let active_paths = Stack::new(allocator, 256);

        storage.push_back_raw([0x5C, 0, 0, 0].as_ptr(), 4);
        storage.push_back(c"_SB_");
        storage.push_back(c"_SB_PCI0");
        roots.push_back((0, 0, 0)); // 0th index is invalid
        roots.push_back((0, 1, 0)); // 1 = Root Path
        roots.push_back((4, 1, 1)); // 2 = _SB_
        roots.push_back((8, 2, 2)); // 3 = _SB_PCI0

        return Self {
            storage,
            frame_stack,
            roots,
            active_paths,
        };
    }

    pub fn get_owner_of(&self, path: u16) -> u16 {
        return self.roots.as_ref(path as u32).unwrap().2;
    }

    pub fn current_path(&self) -> u16 {
        if self.active_paths.num_occupied() != 0 {
            return *self.active_paths.ref_top();
        } else {
            return 1; // Root
        }
    }

    pub fn push_frame(&mut self) {
        if self.frame_stack.num_occupied() != 0 {
            self.frame_stack.mut_top().extended = true;
        }
        self.frame_stack.push(Frame::default());
    }
    pub fn pop_frame(&mut self) {
        let stack = self.frame_stack.pop();
        if let Option::Some(_) = stack.appended {
            *self.active_paths.mut_top() = 0;
            self.active_paths.pop();
        }
        self.frame_stack.mut_top().extended = false;
    }

    pub fn find(&self, base: u16, ptr: *const c_uchar, segments: u8) -> Option<u16> {
        let (base_storage_offset, base_length, _) = self.roots.as_ref(base as u32).unwrap();

        let full_path_length = *base_length + segments;
        let mut ret = Option::None;
        self.roots.for_each(|index, val_| -> bool {
            if index == base as u32 {
                return true;
            }
            let (storage_offset, length, _owner) = unsafe { val_.as_ref().unwrap() };
            if full_path_length != *length {
                return true;
            }
            /* Compares if the beginning is the same*/
            if self.storage.compare(
                *storage_offset,
                *base_storage_offset,
                (*base_length as u16) * 4,
            ) {
                /* Compares if the end is the same*/
                if self.storage.compare_extern(
                    *storage_offset + (*base_length as u16) * 4,
                    ptr,
                    segments as u16 * 4,
                ) {
                    ret = Option::Some(index as u16);
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });
        return ret;
    }
    /**
     * cmp0 => name_system id
     * cmp1 => name_system id
     *
     * compares cmp0 to cmp1 + append
     * True => cmp 0 == cmp1 + append
     */
    pub fn compare_extern(
        &self,
        cmp0: u16,
        cmp1: u16,
        append: *const c_uchar,
        append_length: u16,
    ) -> bool {
        let (cmp0_storage_offset, cmp0_length, _) = self.roots.as_ref(cmp0 as u32).unwrap();
        let (cmp1_storage_offset, cmp1_length, _) = self.roots.as_ref(cmp1 as u32).unwrap();
        if *cmp0_length as u16 * 4 != *cmp1_length as u16 * 4 + append_length {
            return false;
        }
        // Compares the beginnings of cmp0 and cmp1
        if !self.storage.compare(
            *cmp0_storage_offset,
            *cmp1_storage_offset,
            *cmp0_length as u16 * 4,
        ) {
            return false;
        }
        // Compares cmp0 to append
        return self.storage.compare_extern(
            *cmp0_storage_offset + *cmp0_length as u16 * 4,
            append,
            append_length,
        );
    }

    pub fn compare_extern_single(&self, cmp0: u16, cmp1: *const c_uchar, cmp1_length: u16) -> bool {
        let (cmp0_storage_offset, cmp0_length, _) = self.roots.as_ref(cmp0 as u32).unwrap();
        if *cmp0_length as u16 * 4 != cmp1_length {
            return false;
        }
        return self
            .storage
            .compare_extern(*cmp0_storage_offset, cmp1, cmp1_length);
    }

    pub fn compare(&self, cmp0: u16, cmp1: u16) -> bool {
        let (cmp0_storage_offset, cmp0_length, _) = self.roots.as_ref(cmp0 as u32).unwrap();
        let (cmp1_storage_offset, cmp1_length, _) = self.roots.as_ref(cmp1 as u32).unwrap();
        if cmp0_length != cmp1_length {
            return false;
        }
        return self.storage.compare(
            *cmp0_storage_offset,
            *cmp1_storage_offset,
            *cmp0_length as u16 * 4,
        );
    }

    pub fn push_as_appended(&mut self, path: u16) {
        self.active_paths.push(path as u16);
        self.frame_stack.mut_top().appended = Option::Some(true);
    }

    pub fn push_as_inserted(&mut self, path: u16) {
        self.active_paths.push(path as u16);
        self.frame_stack.mut_top().appended = Option::Some(false);
    }

    //TODO: Verify this!
    pub fn append(&mut self, name: &NameString) -> u16 {
        let segments;
        let ptr;
        match name {
            NameString::Single(name, _) => {
                segments = 1;
                ptr = name.as_ptr();
            }
            NameString::Dual(ptr_, _) => {
                segments = 2;
                ptr = *ptr_;
            }
            NameString::Multiple(ptr_, segments_, _) => {
                segments = *segments_;
                ptr = *ptr_;
            }
        }
        //If the full path is allready present => return
        if let Option::Some(base) = self.find(*self.active_paths.ref_top(), ptr, segments) {
            return base;
        }

        let current_index = *self.active_paths.ref_top();

        let (current_offset, current_length, _) = self.roots.as_ref(current_index as u32).unwrap();
        let offset = self.storage.get_length();
        // duplicates the current path
        self.storage
            .copy(offset, *current_offset, (*current_length as u16) * 4);
        // appends 'name' to the duplicated current path
        self.storage.push_back_raw(ptr, (segments as u16) * 4);

        let ret = self.roots.size();
        self.roots
            .push_back((offset, segments + *current_length, current_index));
        // sets the current path as the appended path.
        return ret as u16;
    }
    //TODO: Verify this!
    pub fn insert(&mut self, name: &NameString) -> u16 {
        let segments;
        let ptr;
        match name {
            NameString::Single(name, _) => {
                segments = 1;
                ptr = name.as_ptr();
            }
            NameString::Dual(ptr_, _) => {
                segments = 2;
                ptr = *ptr_;
            }
            NameString::Multiple(ptr_, segments_, _) => {
                segments = *segments_;
                ptr = *ptr_;
            }
        }
        //If the full path is allready present => return.
        if let Option::Some(base) = self.find(0, ptr, segments) {
            return base;
        }
        let offset = self.storage.get_length();
        //Inserts full path into storage
        self.storage.push_back_raw(ptr, (segments as u16) * 4);

        let owner;
        if segments != 1 {
            owner = match self.find(
                0,
                unsafe { self.storage.get_current().sub((segments as usize) * 4) },
                segments - 1,
            ) {
                Some(base) => base,
                None => 0,
            };
        } else {
            owner = 0;
        }

        let ret = self.roots.size();
        self.roots.push_back((offset, segments, owner));
        // sets the current path as the inserted path.
        return ret as u16;
    }
}
