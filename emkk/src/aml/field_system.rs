use crate::{
    aml::definitions::FieldElement,
    hal::{memory::allocator::Allocator, print::simple_kernel_panic},
    utils::allocators::PageAllocator,
};

/**
 * [u16] => offset into entries
 * [u8] => number of entries
 */

type ReferenceBucket = (u16, u8);

/**
 * [Field] => content
 * [u8] => flags
 */
type Entry = (FieldElement, u8);

pub struct FieldSystem {
    systems: PageAllocator<[u16; 2]>,

    reference_buckets: PageAllocator<ReferenceBucket>,

    entries: PageAllocator<Entry>,

    current_flags: u8,
}

impl Default for FieldSystem {
    fn default() -> Self {
        return Self {
            systems: PageAllocator::default(),
            reference_buckets: PageAllocator::default(),
            entries: PageAllocator::default(),
            current_flags: 0,
        };
    }
}

impl FieldSystem {
    pub fn new(allocator: &mut Allocator) -> FieldSystem {
        let mut ret = Self {
            systems: PageAllocator::new(allocator, 256),
            reference_buckets: PageAllocator::new(allocator, 256),
            entries: PageAllocator::new(allocator, 256),
            current_flags: 0,
        };

        ret.reference_buckets.push_back((0, 0)); // marks 0th invalid

        return ret;
    }

    /**
     * Option::None => new reference_bucket has to be addded
     * Option::Some() => reference bucket which can be continued
     */
    fn can_continue(&mut self, name_system: u16) -> Option<&mut ReferenceBucket> {
        let fragments: &[u16; 2] = self.systems.as_ref(name_system as u32).unwrap();

        for i in 0..2 {
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
        let fragments: &mut [u16; 2] = self.systems.as_mut(name_system as u32).unwrap();
        for i in 0..2 {
            if fragments[i] == 0 {
                fragments[i] = self.reference_buckets.size() as u16;
                self.reference_buckets
                    .push_back((self.entries.size() as u16, 0));
                return self.reference_buckets.as_mut(fragments[i] as u32).unwrap();
            }
        }
        simple_kernel_panic(
            "FieldSystem/attach_reference_bucket",
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

    pub fn new_system(&mut self, flags: u8) -> u16 {
        let reference_bucket_index = self.reference_buckets.size();
        self.reference_buckets
            .push_back((self.entries.size() as u16, 0));
        let ret = self.systems.size() as u16;
        self.systems.push_back([reference_bucket_index as u16, 0]);
        self.current_flags = flags;
        return ret;
    }

    pub fn add(&mut self, fs: u16, field_element: FieldElement) {
        if fs > self.systems.size() as u16 {
            simple_kernel_panic("FieldSystem/add", "Invalid FieldSystem\n");
        }
        let current_flags = self.current_flags;
        let entry = self.allocate_new_entry(fs);

        entry.0 = field_element;
        entry.1 = current_flags;
    }

    pub fn set_flags(&mut self, flags: u8) {
        self.current_flags = flags;
    }
}
