use core::{ffi::c_uchar, ptr::null_mut};

use crate::{
    aml::{NameString, definitions::DataRefObject},
    hal::{memory::allocator::Allocator, print::simple_kernel_panic},
    utils::{allocators::PageAllocator, string::String},
};

type ReferenceBucket = (u16, u8);

pub struct Entry {
    pub name: u16, // offset
    pub offset: u16,
}

impl Default for Entry {
    fn default() -> Self {
        return Self { name: 0, offset: 0 };
    }
}

pub struct NameSystem {
    /*
     * Contains all the names, e.g '_HID', '_UID', ...
     */
    names: String,

    /**
     * Each System can be fragmented 4 times
     * Each u16 is the index of a ReferenceBucket
     */
    systems: PageAllocator<[u16; 4]>,

    /**
     *  0th is invalid
     *  u16 => offset into entries
     *  u8 => entry count
     */
    reference_buckets: PageAllocator<ReferenceBucket>,

    entries: PageAllocator<Entry>,

    memory: PageAllocator<DataRefObject>,

    /**
     * This is unsafe, but NameSystem will be used in a kernel stage, where no other core is active.
     * NameSystem is also read only after AmlCode::new() has returned
     */
    allocator: *mut Allocator,
}

impl Default for NameSystem {
    fn default() -> Self {
        return Self {
            names: String::default(),
            systems: PageAllocator::default(),
            reference_buckets: PageAllocator::default(),
            entries: PageAllocator::default(),
            memory: PageAllocator::default(),
            allocator: null_mut(),
        };
    }
}

impl NameSystem {
    pub fn new(allocator: &mut Allocator) -> NameSystem {
        let allocator_as_ptr = allocator as *mut Allocator;

        let mut reference_buckets = PageAllocator::new(allocator, 1024);
        reference_buckets.push_back((0, 0)); // marks 0th invalid

        let mut ret = NameSystem {
            names: String::new(allocator, 0x1000),
            systems: PageAllocator::new(allocator, 1024),
            reference_buckets,
            entries: PageAllocator::new(allocator, 1024),
            memory: PageAllocator::new(allocator, 0x1000),
            allocator: allocator_as_ptr,
        };

        ret.names.push_back(c"_HID");
        ret.names.push_back(c"_UID");
        ret.names.push_back(c"_CID");
        ret.names.push_back(c"_CRS");
        ret.names.push_back(c"_PRS");
        ret.names.push_back(c"_ADR");

        return ret;
    }

    /**
     * Option::None => new reference_bucket has to be addded
     * Option::Some() => reference bucket which can be continued
     */
    fn can_continue(&mut self, name_system: u16) -> Option<&mut ReferenceBucket> {
        let fragments: &[u16; 4] = self.systems.as_ref(name_system as u32).unwrap();

        for i in 0..4 {
            if fragments[i] == 0 {
                continue;
            }
            let reference_bucket: &ReferenceBucket =
                self.reference_buckets.as_ref(fragments[i] as u32).unwrap();
            if reference_bucket.0 + reference_bucket.1 as u16 == self.entries.size() as u16 {
                return Option::Some(self.reference_buckets.as_mut(fragments[i] as u32).unwrap());
            }
        }
        return Option::None;
    }

    /**
     * Attached/Inserted ReferenceBucket will have length=0
     */
    fn attach_reference_bucket<'a>(&'a mut self, name_system: u16) -> &'a mut ReferenceBucket {
        let fragments: &mut [u16; 4] = self.systems.as_mut(name_system as u32).unwrap();
        for i in 0..4 {
            if fragments[i] == 0 {
                fragments[i] = self.reference_buckets.size() as u16;
                self.reference_buckets
                    .push_back((self.entries.size() as u16, 0));
                return self.reference_buckets.as_mut(fragments[i] as u32).unwrap();
            }
        }
        simple_kernel_panic(
            "NameSystem/attach_reference_bucket",
            "Fragmentation Error\n",
        );
    }

    fn allocate_new_entry<'a>(&'a mut self, name_system: u16) -> &'a mut Entry {
        if let Some(ref_bucket) = self.can_continue(name_system) {
            ref_bucket.1 += 1;
        } else {
            self.attach_reference_bucket(name_system).1 = 1;
        }
        let index = self.entries.size();
        self.entries.push_back(Entry::default());
        return self.entries.as_mut(index).unwrap();
    }

    /**
     * Only inserts, when name is not present.
     */
    fn insert_name(&mut self, name: *const c_uchar) -> u16 {
        for i in 0..self.names.get_length() / 4 {
            if self.names.compare_extern(i * 4, name, 4) {
                return i;
            }
        }
        let ret = self.names.get_length() / 4;
        self.names.push_back_raw(name, 4);
        return ret;
    }

    /* TODO: Implement this!*/
    pub fn add(&mut self, name_system: u16, name: &NameString, data_ref_object: &DataRefObject) {
        if name_system > self.systems.size() as u16 {
            simple_kernel_panic("NameSystem/add", "Invalid name_system\n");
        }
        if let NameString::Single(name, _) = name {
            let name_offset = self.insert_name(name.as_ptr());
            let memory_offset = self.memory.size() as u16;

            let entry = self.allocate_new_entry(name_system);
            entry.name = name_offset;
            entry.offset = memory_offset;
            self.memory.push_back(data_ref_object.clone());
        } else {
            simple_kernel_panic("NameSystem/add", "name is not single segment\n")
        }
    }

    pub fn get(&self, name_system: u16, name: [c_uchar; 4]) -> Option<&DataRefObject> {
        if name_system > self.systems.size() as u16 {
            return Option::None;
        }
        let fragments = self.systems.as_ref(name_system as u32).unwrap();
        for i in 0..4 {
            if fragments[i] == 0 {
                break;
            }
            let reference_bucket = self.reference_buckets.as_ref(fragments[i] as u32).unwrap();

            for e in 0..reference_bucket.1 as u16 {
                let entry = self
                    .entries
                    .as_ref((reference_bucket.0 + e) as u32)
                    .unwrap();
                if self.names.compare_extern(entry.name * 4, name.as_ptr(), 4) {
                    return self.memory.as_ref(entry.offset as u32);
                }
            }
        }
        return Option::None;
    }

    pub fn get_mut(&mut self, name_system: u16, name: [c_uchar; 4]) -> Option<&mut DataRefObject> {
        if name_system > self.systems.size() as u16 {
            return Option::None;
        }
        let fragments = self.systems.as_ref(name_system as u32).unwrap();
        for i in 0..4 {
            if fragments[i] == 0 {
                break;
            }
            let reference_bucket = self.reference_buckets.as_ref(fragments[i] as u32).unwrap();

            for e in 0..reference_bucket.1 as u16 {
                let entry = self
                    .entries
                    .as_ref((reference_bucket.0 + e) as u32)
                    .unwrap();
                if self.names.compare_extern(entry.name * 4, name.as_ptr(), 4) {
                    return self.memory.as_mut(entry.offset as u32);
                }
            }
        }
        return Option::None;
    }

    pub fn new_system(&mut self) -> u16 {
        let reference_bucket_index = self.reference_buckets.size() as u16;
        self.reference_buckets
            .push_back((self.entries.size() as u16, 0));
        let ret = self.systems.size();
        self.systems.push_back([reference_bucket_index, 0, 0, 0]);
        return ret as u16;
    }
}
