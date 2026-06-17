use core::{ptr::null_mut, slice};

use crate::{
    hal::memory::allocator::Allocator,
    utils::{allocators::NodeAllocator, invalid_mut_slice},
    vfs::gfs::{link::GfsLink, string_allocator::GfsStringAllocator},
};
#[derive(Clone, Copy)]
pub struct DirectoryHashmapEntry {
    link: GfsLink,
    string_offset: u32,
}

pub struct DirectoryHashmapBucket {
    /* length is always a multiple of 4*/
    entries: &'static mut [DirectoryHashmapEntry],
    used: u8,
}

impl DirectoryHashmapBucket {
    const MAXIMUM_ENTRIES: usize = 16;
    #[inline(always)]
    pub fn append(&mut self, string_offset: u32, link: GfsLink) {
        self.entries[self.used as usize] = DirectoryHashmapEntry {
            string_offset,
            link,
        };
        self.used += 1;
    }
    pub fn contains(&self, manager: &GfsDirectoryHashmapManager, compare_to: &str) -> bool {
        for i in 0..self.used {
            let entry = &self.entries[i as usize];
            if manager
                .string_allocator
                .compare(entry.string_offset, compare_to)
            {
                return true;
            }
        }
        return false;
    }
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        return self.used == self.entries.len() as u8;
    }

    #[inline(always)]
    pub fn can_grow(&self) -> bool {
        return self.entries.len() != Self::MAXIMUM_ENTRIES;
    }
    #[inline(always)]
    pub fn copy_contents_to(&self, entries: &mut [DirectoryHashmapEntry], offset: usize) {
        entries[offset..offset + self.entries.len()].copy_from_slice(self.entries);
    }

    pub fn free_contents(&mut self, allocator: &mut NodeAllocator<DirectoryHashmapEntry>) {
        let offset = (self.entries.as_mut_ptr() as u64 - allocator.get_base())
            / size_of::<DirectoryHashmapEntry>() as u64;

        allocator.free_offset(offset as u32, self.entries.len() as u32);
    }

    pub fn assign(
        &mut self,
        allocator: &NodeAllocator<'_, DirectoryHashmapEntry>,
        offset: u32,
        length: u32,
    ) {
        self.entries = unsafe {
            slice::from_raw_parts_mut(
                (allocator.get_base() as usize
                    + offset as usize * size_of::<DirectoryHashmapEntry>())
                    as *mut DirectoryHashmapEntry,
                length as usize,
            )
        };
    }

    pub fn grow(
        &mut self,
        entry_allocator: &mut NodeAllocator<'_, DirectoryHashmapEntry>,
        general_entries: &mut [DirectoryHashmapEntry],
        growth_rate: u8,
    ) -> bool {
        let new_size = self.entries.len() * growth_rate as usize;
        return match entry_allocator.allocate_as_offset(new_size as u32) {
            Some(new_offset) => {
                self.copy_contents_to(general_entries, new_offset as usize);
                self.free_contents(entry_allocator);
                self.assign(entry_allocator, new_offset, new_size as u32);
                true
            }
            None => false,
        };
    }
}

pub struct GfsDirectoryHashmapManager<'a> {
    buckets: &'a mut [DirectoryHashmapBucket],
    entries: &'a mut [DirectoryHashmapEntry],

    bucket_allocator: NodeAllocator<'a, DirectoryHashmapBucket>,
    entry_allocator: NodeAllocator<'a, DirectoryHashmapEntry>,

    allocator: *mut Allocator,
    allocated_buckets: u32,
    allocated_entries: u32,
    string_allocator: GfsStringAllocator<'a>,
}

impl<'a> GfsDirectoryHashmapManager<'a> {
    pub const fn empty() -> Self {
        return Self {
            buckets: invalid_mut_slice(),
            entries: invalid_mut_slice(),
            entry_allocator: NodeAllocator::empty(),
            bucket_allocator: NodeAllocator::empty(),
            allocator: null_mut(),
            allocated_buckets: 0,
            allocated_entries: 0,
            string_allocator: GfsStringAllocator::empty(),
        };
    }

    const INIT_BUCKET_COUNT: u32 = 640;
    const INIT_ENTRY_COUNT: u32 = Self::INIT_BUCKET_COUNT * 4;

    pub fn new(allocator: &mut Allocator) -> Self {
        let bucket_allocator = NodeAllocator::new(allocator, Self::INIT_BUCKET_COUNT);
        let entry_allocator = NodeAllocator::new(allocator, Self::INIT_ENTRY_COUNT);

        let mut ret = Self {
            buckets: invalid_mut_slice(),
            entries: invalid_mut_slice(),
            bucket_allocator,
            entry_allocator,
            allocator: allocator as *mut Allocator,
            allocated_buckets: 0,
            allocated_entries: 0,
            string_allocator: GfsStringAllocator::new(allocator),
        };

        unsafe {
            ret.buckets = slice::from_raw_parts_mut(
                ret.bucket_allocator.get_base() as *mut DirectoryHashmapBucket,
                Self::INIT_BUCKET_COUNT as usize,
            );
            ret.entries = slice::from_raw_parts_mut(
                ret.entry_allocator.get_base() as *mut DirectoryHashmapEntry,
                Self::INIT_ENTRY_COUNT as usize,
            );
        }

        return ret;
    }

    pub fn allocate_hashmap(&mut self, num_entries: u32) -> GfsDirectoryHashmap {
        // size checks and reallocation
        {
            if self.allocated_buckets + num_entries > self.buckets.len() as u32 {
                self.bucket_allocator
                    .reallocate(unsafe { self.allocator.as_mut().unwrap() }, 2);
                self.buckets = self.bucket_allocator.as_mut_slice();
            }
            /* each bucket can contain 4 directory entries at start*/
            if self.allocated_entries + num_entries * 4 > self.entries.len() as u32 {
                let prev_base = self.entry_allocator.get_base();
                self.entry_allocator
                    .reallocate(unsafe { self.allocator.as_mut().unwrap() }, 4);
                let base = self.entry_allocator.get_base();
                for i in 0..self.buckets.len() {
                    let bucket = &mut self.buckets[i];
                    let len = bucket.entries.len();
                    bucket.entries = unsafe {
                        slice::from_raw_parts_mut(
                            (base + (bucket.entries.as_ptr() as u64 - prev_base))
                                as *mut DirectoryHashmapEntry,
                            len,
                        )
                    };
                }
            }
        }

        let bucket_offset = self
            .bucket_allocator
            .allocate_as_offset(num_entries)
            .unwrap();
        let entries_offset = self
            .entry_allocator
            .allocate_as_offset(num_entries * 4)
            .unwrap();

        let buckets = self.bucket_allocator.as_mut_slice();
        let entries = self.entry_allocator.as_mut_slice();
        for i in 0..num_entries {
            let bucket = &mut buckets[bucket_offset as usize + i as usize];

            let bucket_entries = &mut entries[entries_offset as usize + i as usize * 4
                ..entries_offset as usize + (i + 1) as usize * 4]
                as *mut [DirectoryHashmapEntry];

            bucket.used = 0;
            bucket.entries = unsafe { bucket_entries.as_mut().unwrap() };
        }

        return GfsDirectoryHashmap {
            bucket_offset,
            num_buckets: num_entries,
            num_entries_used: num_entries * 4,
        };
    }
}

pub enum GfsDirectoryHashmapInsertion {
    AllreadyPresent,
    EntryReallocationRequired,
    StringReallocationRequired,
    BucketReallocationRequired,
    Success { rehashed: bool },
}

pub struct GfsDirectoryHashmap {
    bucket_offset: u32,
    num_buckets: u32,
    num_entries_used: u32,
}
impl GfsDirectoryHashmap {
    /* temporary implementation*/
    fn hash_str(&self, name: &str, bucket_count: u32) -> u32 {
        let mut hash_value: u64 = 0;

        for b in name.as_bytes() {
            hash_value = hash_value.wrapping_add(*b as u64 * 577);
            hash_value = hash_value.wrapping_mul(11) % self.num_buckets as u64;
        }
        assert!(self.num_buckets - 1 >= hash_value as u32);
        return hash_value as u32;
    }

    pub fn for_each<'a>(
        &self,
        manager: &'a GfsDirectoryHashmapManager<'a>,
        mut func: impl FnMut(&'a str, &GfsLink),
    ) {
        for i in 0..self.num_buckets {
            let bucket = &manager.buckets[self.bucket_offset as usize + i as usize];
            for j in 0..bucket.used as usize {
                let entry = &bucket.entries[j as usize];
                let entry_name = manager.string_allocator.as_string(entry.string_offset);
                (func)(entry_name, &entry.link);
            }
        }
    }

    fn rehash_bucket(
        &self,
        old_bucket: &DirectoryHashmapBucket,
        new_buckets: &mut [DirectoryHashmapBucket],
        string_allocator: &mut GfsStringAllocator,
        entry_allocator: &mut NodeAllocator<'_, DirectoryHashmapEntry>,
        general_entries: &mut [DirectoryHashmapEntry],
    ) -> bool {
        for i in 0..old_bucket.entries.len() {
            let entry: &DirectoryHashmapEntry = &old_bucket.entries[i];
            let new_bucket_index = self.hash_str(
                string_allocator.as_string(entry.string_offset),
                self.num_buckets * 2,
            );

            let dst_bucket = &mut new_buckets[new_bucket_index as usize];
            if dst_bucket.is_full() {
                assert_eq!(dst_bucket.can_grow(), true);
                if !dst_bucket.grow(entry_allocator, general_entries, 2) {
                    return false;
                }
            }
            dst_bucket.append(entry.string_offset, entry.link);
        }
        return true;
    }

    fn rehash_core(
        &mut self,
        manager: &mut GfsDirectoryHashmapManager,
        new_bucket_offset: u32,
        entry_offset: u32,
    ) -> GfsDirectoryHashmapInsertion {
        let new_buckets = unsafe {
            slice::from_raw_parts_mut(
                manager.buckets.as_mut_ptr().add(new_bucket_offset as usize),
                self.num_buckets as usize * 2,
            )
        };
        for (bucket_index, bucket) in new_buckets.iter_mut().enumerate() {
            bucket.used = 0;

            bucket.assign(
                &manager.entry_allocator,
                entry_offset + bucket_index as u32 * 4,
                4,
            );
        }

        let old_buckets = &mut manager.buckets
            [self.bucket_offset as usize..(self.bucket_offset + self.num_buckets) as usize];
        for i in 0..old_buckets.len() {
            let old_bucket = &mut old_buckets[i as usize];

            if !self.rehash_bucket(
                old_bucket,
                new_buckets,
                &mut manager.string_allocator,
                &mut manager.entry_allocator,
                &mut manager.entries,
            ) {
                for i in 0..new_buckets.len() {
                    let bucket: &mut DirectoryHashmapBucket = &mut new_buckets[i];
                    bucket.free_contents(&mut manager.entry_allocator);
                }
                manager
                    .bucket_allocator
                    .free_offset(new_bucket_offset, new_buckets.len() as u32);
                return GfsDirectoryHashmapInsertion::EntryReallocationRequired;
            }
        }

        for i in 0..old_buckets.len() {
            let bucket: &mut DirectoryHashmapBucket = &mut old_buckets[i];
            bucket.free_contents(&mut manager.entry_allocator);
        }
        manager
            .bucket_allocator
            .free_offset(self.bucket_offset, old_buckets.len() as u32);
        self.bucket_offset = new_bucket_offset;
        self.num_buckets *= 2;
        return GfsDirectoryHashmapInsertion::Success { rehashed: true };
    }

    fn rehash(&mut self, manager: &mut GfsDirectoryHashmapManager) -> GfsDirectoryHashmapInsertion {
        let bucket_allocation = manager
            .bucket_allocator
            .allocate_as_offset(self.num_buckets * 2);
        let entry_allocation = manager
            .entry_allocator
            .allocate_as_offset(self.num_entries_used * 2);

        /* this can be optimize by nesting it inside a match block*/
        let bucket_offset;
        let entry_offset;
        match bucket_allocation {
            Some(bucket_off) => bucket_offset = bucket_off,
            None => {
                if let Some(off) = entry_allocation {
                    manager
                        .entry_allocator
                        .free_offset(off, self.num_entries_used * 2);
                }
                return GfsDirectoryHashmapInsertion::BucketReallocationRequired;
            }
        }

        match entry_allocation {
            Some(entry_off) => entry_offset = entry_off,
            None => {
                manager
                    .bucket_allocator
                    .free_offset(bucket_offset, self.num_buckets * 2);
                return GfsDirectoryHashmapInsertion::EntryReallocationRequired;
            }
        }
        self.num_entries_used *= 2;
        return self.rehash_core(manager, bucket_offset, entry_offset);
    }

    pub fn insert(
        &mut self,
        manager: &mut GfsDirectoryHashmapManager,
        name: &str,
        link: GfsLink,
    ) -> GfsDirectoryHashmapInsertion {
        let mut bucket_index = self.hash_str(name, self.num_buckets);
        let mut rehashed = false;

        let full;
        let growable;
        let length;
        {
            let bucket = &manager.buckets[(self.bucket_offset + bucket_index) as usize];
            full = bucket.is_full();
            growable = bucket.can_grow();
            length = bucket.entries.len();
        }

        if full {
            if !growable {
                let n = self.rehash(manager);
                match n {
                    GfsDirectoryHashmapInsertion::EntryReallocationRequired
                    | GfsDirectoryHashmapInsertion::BucketReallocationRequired
                    | GfsDirectoryHashmapInsertion::StringReallocationRequired => {
                        self.num_entries_used /= 2;
                        return n;
                    }
                    _ => {}
                };
                bucket_index = self.hash_str(name, self.num_buckets);
                rehashed = true;
            }

            if !manager.buckets[(self.bucket_offset + bucket_index) as usize].grow(
                &mut manager.entry_allocator,
                &mut manager.entries,
                2,
            ) {
                return GfsDirectoryHashmapInsertion::EntryReallocationRequired;
            }

            self.num_entries_used += length as u32;
        }

        if manager.buckets[(self.bucket_offset + bucket_index) as usize].contains(manager, name) {
            return GfsDirectoryHashmapInsertion::AllreadyPresent;
        }

        let bucket = &mut manager.buckets[(self.bucket_offset + bucket_index) as usize];
        match manager.string_allocator.allocate(name) {
            Some(string_offset) => bucket.append(string_offset, link),
            None => return GfsDirectoryHashmapInsertion::StringReallocationRequired,
        }

        return GfsDirectoryHashmapInsertion::Success { rehashed };
    }
    pub fn lookup(&self, manager: &GfsDirectoryHashmapManager, name: &str) -> Option<GfsLink> {
        let bucket_index = self.hash_str(name, self.num_buckets);
        let bucket = &manager.buckets[self.bucket_offset as usize + bucket_index as usize];
        for i in 0..bucket.used {
            let entry = &bucket.entries[i as usize];
            if manager.string_allocator.compare(entry.string_offset, name) {
                return Option::Some(entry.link.clone());
            }
        }
        return Option::None;
    }
}
