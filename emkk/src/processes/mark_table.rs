use core::slice;

use crate::{
    fixed_vaddrs::{
        FIXED_PROCESSOR_VIRTUAL_ADDRESS, PROCESSES_MARK_TABLE_BUCKETS_FIXED_VADDR,
        PROCESSES_MARK_TABLE_PAIR_FIXED_VADDR, ref_processor_mut,
    },
    hal::{
        memory::{
            allocator::{Allocator, MemoryBlock},
            pager::{PAGER_PRESENT, PAGER_RW},
        },
        print::simple_kernel_panic,
    },
    multithreading::processors::Processor,
    utils::{allocators::NodeAllocator, slices::resize_slice, traits::Region},
};
#[repr(align(8))]
pub struct MarkTableBucket {
    /* key, index to reference*/
    pairs: &'static mut [(u32, u32)],
    used: u32,
}

impl MarkTableBucket {
    pub const MAXIMUM_REFERENCES: usize = 8;
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        return self.used == self.pairs.len() as u32;
    }
    #[inline(always)]
    pub fn can_grow(&self) -> bool {
        return self.pairs.len() != Self::MAXIMUM_REFERENCES;
    }

    pub fn contains(&self, key: u32) -> bool {
        for i in 0..self.used as usize {
            if self.pairs[i].0 == key {
                return true;
            }
        }
        return false;
    }

    pub fn append_pair(&mut self, reference: (u32, u32)) {
        self.pairs[self.used as usize] = reference;
        self.used += 1;
    }

    pub fn foreach_mut(&mut self, mut func: impl FnMut(u32, &mut u32) -> bool) {
        for i in 0..self.used as usize {
            let (e0, e1) = &mut self.pairs[i as usize].clone();
            if !(func)(*e0, e1) {
                return;
            }
        }
    }

    pub fn foreach(&self, mut func: impl FnMut(u32, u32)) {
        for i in 0..self.used as usize {
            let (e0, e1) = self.pairs[i as usize].clone();
            (func)(e0, e1);
        }
    }
}

pub struct MarkTable {
    pairs: &'static mut [(u32, u32)],
    buckets: &'static mut [MarkTableBucket],

    pair_allocator: NodeAllocator<'static, (u32, u32)>,

    /* TODO: this! No NodeAllocator for Buckets, since buckets are permanent!*/
    num_pairs: u32,
}

pub enum MarkTableResult {
    Success,
    ValueKeyReallocationRequired,
    RehashRequired,
    NotFound,
}

impl MarkTable {
    pub fn new(allocator: &mut Allocator) -> Self {
        let value_key_memory = allocator
            .alloc_zero(((4096 * size_of::<(u32, u32)>()) / 0x1000) as u16)
            .unwrap();
        let bucket_memory = allocator
            .alloc_zero(((1024 * size_of::<MarkTableBucket>()) / 0x1000) as u16)
            .unwrap();

        let pager = unsafe { &mut *ref_processor_mut().pager };

        if value_key_memory
            .map(
                allocator,
                pager,
                PROCESSES_MARK_TABLE_PAIR_FIXED_VADDR,
                PAGER_RW | PAGER_PRESENT,
            )
            .is_some()
        {
            simple_kernel_panic("MarkTable/new", "Could not map references\n");
        }

        if bucket_memory
            .map(
                allocator,
                pager,
                PROCESSES_MARK_TABLE_BUCKETS_FIXED_VADDR,
                PAGER_RW | PAGER_PRESENT,
            )
            .is_some()
        {
            simple_kernel_panic("MarkTable/new", "Could not map buckets\n");
        }
        let mut pair_allocator = NodeAllocator::new_fake(
            allocator,
            &MemoryBlock::new(
                value_key_memory.length,
                PROCESSES_MARK_TABLE_PAIR_FIXED_VADDR,
            ),
            1024,
        );

        let pairs;
        let buckets;

        unsafe {
            pairs = slice::from_raw_parts_mut(
                PROCESSES_MARK_TABLE_PAIR_FIXED_VADDR as *mut (u32, u32),
                1024,
            );
            buckets = slice::from_raw_parts_mut(
                PROCESSES_MARK_TABLE_BUCKETS_FIXED_VADDR as *mut MarkTableBucket,
                1024,
            );

            for i in 0..buckets.len() {
                let offset = pair_allocator.allocate_as_offset(2).unwrap();
                buckets[i as usize].used = 0;
                buckets[i as usize].pairs =
                    slice::from_raw_parts_mut(pairs.as_mut_ptr().add(offset as usize), 2);
            }
        }

        return Self {
            pairs,
            buckets,
            pair_allocator,
            num_pairs: 1024,
        };
    }

    fn hash(fi: u32, size: u32) -> u32 {
        let mut key = fi as u64;
        key = (!key).wrapping_add(key << 15);
        key = key ^ (key >> 10);
        key = key.wrapping_add(key << 3);
        key = key ^ (key >> 6);
        key = (!key).wrapping_add(key << 11);
        key = key ^ (key >> 16);
        return (key & size as u64) as u32;
    }

    pub fn contains(&self, fi: u32) -> bool {
        let bucket_index = MarkTable::hash(fi, self.buckets.len() as u32);
        let bucket = &self.buckets[bucket_index as usize];
        return bucket.contains(fi);
    }

    pub fn rehash(&mut self, allocator: &mut Allocator) -> MarkTableResult {
        /* TODO: test this!*/
        let mut new_pairs = self.num_pairs * 2;
        if new_pairs % 4096 != 0 {
            new_pairs += 4096 - (new_pairs % 4096);
        }

        let mut pair_allocator: NodeAllocator<'static, (u32, u32)> =
            NodeAllocator::new(allocator, new_pairs);

        let new_pairs = match self
            .pair_allocator
            .allocate_as_offset(self.buckets.len() as u32 * 2)
        {
            Some(offset) => offset,
            None => {
                self.pair_allocator.reallocate(allocator, 2);

                match self
                    .pair_allocator
                    .allocate_as_offset(self.buckets.len() as u32 * 2)
                {
                    Some(offset) => offset,
                    None => simple_kernel_panic("MarkTable/rehash", "Could not allocate new pairs"),
                }
            }
        };

        let bucket_mb = allocator
            .alloc_zero((((self.buckets.len() * size_of::<MarkTableBucket>()) / 0x1000) * 2) as u16)
            .unwrap();

        let new_buckets = unsafe {
            slice::from_raw_parts_mut(
                bucket_mb.base as *mut MarkTableBucket,
                self.buckets.len() * 2,
            )
        };
        for i in 0..self.buckets.len() {
            let bucket = &self.buckets[i as usize];
            bucket.foreach(|key, count| {
                let new_index = MarkTable::hash(key, self.buckets.len() as u32 * 2);
                let dst_bucket = &mut new_buckets[new_index as usize];

                if dst_bucket.is_full() {
                    if !dst_bucket.can_grow() {
                        simple_kernel_panic("MarkTable/rehash", "Bucket cannot grow\n")
                    }
                    let new_size;
                    if dst_bucket.pairs.len() == 0 {
                        new_size = 2;
                        self.num_pairs += 2;
                    } else {
                        new_size = dst_bucket.pairs.len() * 2;
                        self.num_pairs += dst_bucket.pairs.len() as u32;
                    }
                    let pair_offset = pair_allocator.allocate_as_offset(new_size as u32).unwrap();
                    let pairs = unsafe {
                        slice::from_raw_parts_mut(
                            (pair_allocator.get_base() as *mut (u32, u32))
                                .add(pair_offset as usize),
                            new_size,
                        )
                    };
                    pairs[0..dst_bucket.pairs.len()].copy_from_slice(dst_bucket.pairs);
                    if dst_bucket.pairs.len() != 0 {
                        if !pair_allocator.free_offset(
                            unsafe {
                                dst_bucket.pairs.as_ptr().offset_from_unsigned(
                                    pair_allocator.get_base() as *const (u32, u32),
                                ) as u32
                            },
                            dst_bucket.pairs.len() as u32,
                        ) {
                            simple_kernel_panic("MarkTable/rehash", "Cannot free old pairs\n")
                        }
                    }
                    dst_bucket.pairs = pairs;
                }
                dst_bucket.append_pair((key, count));
            });
        }

        let pager = unsafe { &mut *ref_processor_mut().pager };

        let bucket_phys = pager
            .get_physical(PROCESSES_MARK_TABLE_BUCKETS_FIXED_VADDR)
            .unwrap();

        let free_bucket_memory = MemoryBlock::new(
            (self.buckets.len() * size_of::<MarkTableBucket>()) as u64,
            bucket_phys,
        );
        allocator.free(&free_bucket_memory).unwrap();
        bucket_mb
            .map(
                allocator,
                pager,
                PROCESSES_MARK_TABLE_BUCKETS_FIXED_VADDR,
                PAGER_PRESENT | PAGER_RW,
            )
            .unwrap();

        let pair_phys = pager
            .get_physical(PROCESSES_MARK_TABLE_PAIR_FIXED_VADDR)
            .unwrap();

        let free_pair_memory = MemoryBlock::new(
            (self.buckets.len() * size_of::<(u32, u32)>()) as u64,
            pair_phys,
        );
        allocator.free(&free_pair_memory).unwrap();
        self.pair_allocator = pair_allocator.to_fake(PROCESSES_MARK_TABLE_PAIR_FIXED_VADDR);

        let new_size = self.buckets.len() * 2;

        resize_slice(&mut self.buckets, new_size);
        resize_slice(&mut self.pairs, new_pairs as usize);
        return MarkTableResult::Success;
    }

    pub fn insert(&mut self, fi: u32) -> MarkTableResult {
        let bucket_index = MarkTable::hash(fi, self.buckets.len() as u32);
        let prev_size = self.buckets[bucket_index as usize].pairs.len();

        if self.buckets[bucket_index as usize].is_full() {
            if !self.buckets[bucket_index as usize].can_grow() {
                return MarkTableResult::RehashRequired;
            } else {
                let bucket = &mut self.buckets[bucket_index as usize];
                let new_entries: &'static mut [(u32, u32)];

                match self
                    .pair_allocator
                    .allocate_as_offset(bucket.pairs.len() as u32 * 2)
                {
                    Some(offset) => {
                        new_entries = unsafe {
                            slice::from_raw_parts_mut(
                                self.pairs.as_mut_ptr().add(offset as usize),
                                bucket.pairs.len() * 2,
                            )
                        };
                    }
                    None => return MarkTableResult::ValueKeyReallocationRequired,
                };
                new_entries[..prev_size].copy_from_slice(bucket.pairs);

                if !self.pair_allocator.free_offset(
                    unsafe {
                        bucket
                            .pairs
                            .as_ptr()
                            .offset_from_unsigned(self.pairs.as_ptr())
                    } as u32,
                    prev_size as u32,
                ) {
                    simple_kernel_panic("MarkTable/insert", "Could not free old pairs\n")
                }
                self.num_pairs += prev_size as u32;
                bucket.pairs = new_entries;
            }
        }
        let bucket = &mut self.buckets[bucket_index as usize];
        bucket.append_pair((fi, 1));
        return MarkTableResult::Success;
    }
    pub fn increase(&mut self, fi: u32) -> MarkTableResult {
        let bucket_index = MarkTable::hash(fi, self.buckets.len() as u32);
        let bucket = &mut self.buckets[bucket_index as usize];
        let mut ret = MarkTableResult::NotFound;
        bucket.foreach_mut(|key, count| {
            if key == fi {
                *count = *count + 1;
                ret = MarkTableResult::Success;
                false;
            }
            true
        });

        return ret;
    }
}
