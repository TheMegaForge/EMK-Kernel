use core::slice;

use crate::{
    fixed_vaddrs::{GFS_STRINGS_FIXED_VADDR, ref_processor_mut},
    hal::memory::{
        allocator::{Allocator, MemoryBlock},
        pager::{PAGER_PRESENT, PAGER_RW},
    },
    multithreading::processors::Processor,
    utils::{allocators::NodeAllocator, rebase_slice, resize_slice, traits::Region},
    vfs::gfs::GFS_STRING_ALLOCATED_SIZE,
};

pub struct GfsStringAllocator<'a> {
    memory: [MemoryBlock; 3],
    _lengths: [&'static mut [u8]; 4],
    bitmaps: [&'a mut [u8]; 3],

    allocator: NodeAllocator<'a, u8>,
}

pub struct GfsStringAllocatorReallocationInfo {
    length16_old_offset: u32,
    length32_old_offset: u32,
    above32_old_offset: u32,
    length16_new_offset: u32,
    length32_new_offset: u32,
    above32_new_offset: u32,
}

impl GfsStringAllocatorReallocationInfo {
    /* old_in indicates 8 bytes string, the offset does not change, since 8 is always at the begining*/
    pub fn assign_new_offset(&self, old_in: u32) -> u32 {
        if self.length16_old_offset > old_in {
            return old_in;
        } else if self.length16_old_offset >= old_in && self.length32_old_offset > old_in {
            return self.length16_new_offset + (old_in - self.length16_old_offset);
        } else if self.length32_old_offset >= old_in && self.above32_old_offset > old_in {
            return self.length32_new_offset + (old_in - self.length32_old_offset);
        } else {
            return self.above32_new_offset + (old_in - self.above32_old_offset);
        }
    }
}

pub enum GfsStringType {
    Length8,
    Length16,
    Length32,
    Above32,
}

impl<'a> GfsStringAllocator<'a> {
    fn string_type(&self, string_offset: u32) -> GfsStringType {
        let base_16 = self._lengths[0].len() as u32;
        let base_32 = (self._lengths[0].len() + self._lengths[1].len()) as u32;
        let base_above32 =
            (self._lengths[0].len() + self._lengths[1].len() + self._lengths[2].len()) as u32;

        if base_16 > string_offset {
            return GfsStringType::Length8;
        } else if base_32 > string_offset && string_offset >= base_16 {
            return GfsStringType::Length16;
        } else if base_above32 > string_offset && string_offset >= base_32 {
            return GfsStringType::Length32;
        } else {
            return GfsStringType::Above32;
        }
    }

    pub const fn empty() -> Self {
        return Self {
            memory: [const { MemoryBlock::empty() }; 3],
            _lengths: unsafe {
                [
                    slice::from_raw_parts_mut(align_of::<u8>() as *mut u8, 0),
                    slice::from_raw_parts_mut(align_of::<u8>() as *mut u8, 0),
                    slice::from_raw_parts_mut(align_of::<u8>() as *mut u8, 0),
                    slice::from_raw_parts_mut(align_of::<u8>() as *mut u8, 0),
                ]
            },
            bitmaps: unsafe {
                [
                    slice::from_raw_parts_mut(align_of::<u8>() as *mut u8, 0),
                    slice::from_raw_parts_mut(align_of::<u8>() as *mut u8, 0),
                    slice::from_raw_parts_mut(align_of::<u8>() as *mut u8, 0),
                ]
            },
            allocator: NodeAllocator::empty(),
        };
    }

    pub fn compare(&self, offset: u32, compare_to: &str) -> bool {
        return self.as_string(offset) == compare_to;
    }

    pub fn as_string(&self, offset: u32) -> &str {
        let max_length;
        let lengths;
        let internal_offset;
        match self.string_type(offset) {
            GfsStringType::Length8 => {
                max_length = 8;
                lengths = &self._lengths[0];
                internal_offset = offset;
            }
            GfsStringType::Length16 => {
                max_length = 16;
                lengths = &self._lengths[1];
                internal_offset = offset - self._lengths[0].len() as u32;
            }
            GfsStringType::Length32 => {
                max_length = 32;
                lengths = &self._lengths[2];
                internal_offset = offset - (self._lengths[0].len() + self._lengths[1].len()) as u32;
            }
            GfsStringType::Above32 => {
                max_length = 255;
                lengths = &self._lengths[3];
                internal_offset = offset
                    - (self._lengths[0].len() + self._lengths[1].len() + self._lengths[2].len())
                        as u32;
            }
        };
        let mut len = 0;
        let str =
            &lengths[internal_offset as usize..internal_offset as usize + max_length as usize];
        for i in 0..max_length {
            if str[i] == 0 {
                break;
            } else {
                len += 1;
            }
        }
        return unsafe {
            str::from_utf8_unchecked(slice::from_raw_parts(
                lengths.as_ptr().add(internal_offset as usize),
                len,
            ))
        };
    }

    pub fn allocate(&mut self, string: &str) -> Option<u32> {
        let length = string.len();
        let offset;
        let bitmap;
        let maximum_length: usize;
        let dst_base;

        match length {
            n if 8 >= n => {
                bitmap = unsafe { (self.bitmaps[0] as *mut [u8]).as_mut().unwrap() };
                maximum_length = 8;
                offset = 0;
                dst_base = unsafe { (self._lengths[0] as *mut [u8]).as_mut().unwrap() };
            }
            n if 16 >= n => {
                bitmap = unsafe { (self.bitmaps[1] as *mut [u8]).as_mut().unwrap() };
                maximum_length = 16;
                offset = self._lengths[0].len();
                dst_base = unsafe { (self._lengths[1] as *mut [u8]).as_mut().unwrap() };
            }
            n if 32 >= n => {
                bitmap = unsafe { (self.bitmaps[2] as *mut [u8]).as_mut().unwrap() };
                maximum_length = 32;
                offset = self._lengths[0].len() + self._lengths[1].len();
                dst_base = unsafe { (self._lengths[2] as *mut [u8]).as_mut().unwrap() };
            }
            _ => {
                let ret = self.allocator.allocate_as_offset(length as u32 + 1);
                if let Some(offset) = ret {
                    let dst = &mut self._lengths[3][offset as usize..offset as usize + length + 1];
                    dst.copy_from_slice(string.as_bytes());
                    dst[length] = '\0' as u8;
                    return Option::Some(
                        offset
                            + (self._lengths[0].len()
                                + self._lengths[1].len()
                                + self._lengths[2].len()) as u32,
                    );
                }
                return Option::None;
            }
        }

        for (entry_index, entry) in bitmap.iter_mut().enumerate() {
            if *entry != 0xFF {
                for b in 0..8usize {
                    if *entry & (1 << b) == 0 {
                        *entry |= 1 << b;
                        let internal_offset: usize =
                            (b * maximum_length) + entry_index * (8 * maximum_length);
                        let dst = &mut dst_base[internal_offset..internal_offset + maximum_length];
                        dst[0..length].copy_from_slice(string.as_bytes());
                        if length != maximum_length {
                            let rem = maximum_length - length;
                            dst[length..length + rem].fill(0);
                        }
                        return Option::Some(offset as u32 + internal_offset as u32);
                    }
                }
            }
        }
        return Option::None;
    }

    pub fn free(&mut self, offset: u32, old_length: u32) {
        self.rename(offset, old_length, unsafe {
            str::from_utf8_unchecked(slice::from_raw_parts(0x1 as *const u8, 0))
        });
    }
    /* NOTICE: to_size = 0 => freeing entry*/
    pub fn rename(&mut self, offset: u32, old_length: u32, new_data: &str) -> Option<u32> {
        let new_length = new_data.len();
        let bitmap;
        let data;
        let internal_offset;
        let maximum_size;
        let minimum_size;

        match self.string_type(offset) {
            GfsStringType::Length8 => {
                assert!(offset % 8 == 0);
                bitmap = &mut self.bitmaps[0];
                data = &mut self._lengths[0];
                internal_offset = offset as usize;
                maximum_size = 8;
                minimum_size = 1;
            }
            GfsStringType::Length16 => {
                assert!(offset % 16 == 0);
                internal_offset = offset as usize - self._lengths[0].len();
                bitmap = &mut self.bitmaps[1];
                data = &mut self._lengths[1];
                maximum_size = 16;
                minimum_size = 9;
            }
            GfsStringType::Length32 => {
                assert!(offset % 32 == 0);
                internal_offset =
                    offset as usize - (self._lengths[0].len() + self._lengths[1].len());
                bitmap = &mut self.bitmaps[2];
                data = &mut self._lengths[2];
                maximum_size = 32;
                minimum_size = 17;
            }
            GfsStringType::Above32 => {
                let internal_offset = offset as usize
                    - (self._lengths[0].len() + self._lengths[1].len() + self._lengths[2].len());
                self.allocator
                    .free_offset(internal_offset as u32, old_length);
                if 32 >= new_length || new_length == 0 {
                    if 32 >= new_length {
                        return self.allocate(new_data);
                    } else {
                        return Option::Some(0);
                    }
                } else {
                    let offset = self.allocator.allocate_as_offset(new_length as u32);
                    if let Some(off) = offset {
                        self._lengths[3][off as usize..off as usize + new_length]
                            .copy_from_slice(new_data.as_bytes());
                        return Option::Some(off);
                    } else {
                        return Option::None;
                    }
                }
            }
        }

        if new_length > maximum_size || minimum_size > new_length || new_length == 0 {
            /* If freed, oversized or undersized*/
            let entry = internal_offset / 8;
            bitmap[entry as usize / 8] ^= 1 << (entry % 8);
            if new_length != 0 {
                return self.allocate(new_data);
            } else {
                return Option::Some(0);
            }
        } else {
            if old_length as usize > new_length {
                /* clears out of scope bytes */
                data[internal_offset + new_length..internal_offset + old_length as usize].fill(0);
            }
            data[internal_offset..internal_offset + new_length]
                .copy_from_slice(new_data.as_bytes());
            return Option::Some(offset);
        }
    }

    fn expand_virtual(allocator: &mut Allocator, new_pages: u16, expand_begin: u64) {
        let mb = allocator.alloc_zero(new_pages).unwrap();

        let pager = ref_processor_mut().ref_mut_pager();

        for i in 0..new_pages {
            pager
                .page_4_kb(
                    expand_begin + i as u64 * 0x1000,
                    mb.base + i as u64 * 0x1000,
                    PAGER_PRESENT | PAGER_RW,
                    allocator,
                )
                .unwrap();
        }
    }

    pub fn reallocate_expand_bitmap(
        bitmap_memory: &mut MemoryBlock,
        bitmap: &mut &'a mut [u8],
        allocator: &mut Allocator,
        new_pages: usize,
        maximum_length: u64,
        growth_factor: usize,
    ) {
        let num_total_entries = bitmap.len() * 8;
        let required_entries = (new_pages * 0x1000) / maximum_length as usize;
        if required_entries > num_total_entries {
            let num_bytes = bitmap.len() * growth_factor;
            let mut num_pages = num_bytes / 0x1000;
            if num_bytes % 0x1000 != 0 {
                num_pages += 1;
            }
            let new_bitmap_memory = allocator.alloc_zero(num_pages as u16).unwrap();
            unsafe {
                let new_bitmap = slice::from_raw_parts_mut(
                    new_bitmap_memory.as_mut_ptr(),
                    (new_bitmap_memory.length / maximum_length) as usize / 8,
                );
                new_bitmap.copy_from_slice(bitmap);
                *bitmap = new_bitmap;
            }
            *bitmap_memory = new_bitmap_memory;
        }
    }

    /*
     * string is used to reallocate a certain length
     */
    pub fn reallocate(
        &mut self,
        allocator: &mut Allocator,
        string: &str,
    ) -> GfsStringAllocatorReallocationInfo {
        let length = string.len();
        let bitmap;
        let bitmap_memory;
        let lengths_index;
        let growth_factor;
        let maximum_length;

        let length16_old_offset = self._lengths[0].len() as u32;
        let length32_old_offset = (self._lengths[0].len() + self._lengths[1].len()) as u32;
        let above32_old_offset =
            (self._lengths[0].len() + self._lengths[1].len() + self._lengths[2].len()) as u32;

        match length {
            n if 8 >= n => {
                bitmap = &mut self.bitmaps[0];
                bitmap_memory = &mut self.memory[0];
                growth_factor = 2;
                maximum_length = 8;
                lengths_index = 0;
            }
            n if 16 >= n => {
                bitmap = &mut self.bitmaps[1];
                bitmap_memory = &mut self.memory[1];
                growth_factor = 4;
                maximum_length = 16;
                lengths_index = 1;
            }
            n if 32 >= n => {
                bitmap = &mut self.bitmaps[2];
                bitmap_memory = &mut self.memory[2];
                growth_factor = 2;
                maximum_length = 32;
                lengths_index = 2;
            }
            _ => {
                self.allocator.reallocate(allocator, 2);
                let new_size = self._lengths[3].len() * 2;
                resize_slice(&mut self._lengths[3], new_size);

                /* old = new, since the last thing got reallocated. Meaning the Posititions are still intact*/
                return GfsStringAllocatorReallocationInfo {
                    length16_old_offset,
                    length32_old_offset,
                    above32_old_offset,
                    length16_new_offset: length16_old_offset,
                    length32_new_offset: length32_old_offset,
                    above32_new_offset: above32_old_offset,
                };
            }
        }

        let original_pages = self._lengths[lengths_index].len() / 0x1000;
        let new_pages = original_pages * growth_factor;
        unsafe {
            GFS_STRING_ALLOCATED_SIZE += new_pages as u32 * 0x1000;
        }
        GfsStringAllocator::expand_virtual(
            allocator,
            new_pages as u16,
            GFS_STRINGS_FIXED_VADDR + above32_old_offset as u64 + self._lengths[3].len() as u64,
        );
        GfsStringAllocator::reallocate_expand_bitmap(
            bitmap_memory,
            bitmap,
            allocator,
            new_pages,
            maximum_length,
            growth_factor,
        );
        {
            /* updates length */
            resize_slice(&mut self._lengths[lengths_index], new_pages * 0x1000);
            /* updates position*/
            for i in lengths_index + 1..4 {
                let new_address = self._lengths[i].as_mut_ptr() as u64 + new_pages as u64 * 0x1000;
                rebase_slice(&mut self._lengths[i], new_address);
            }
            /*
             * changes the address of the above32 allocator, so that all future allocations will be correct
             */
            self.allocator.change_base(self._lengths[3].as_ptr() as u64);
        }
        {
            /* copying in reverse order to prevent data corruption*/
            for i in 3..=lengths_index + 1 {
                let effected = &mut self._lengths[i as usize];
                let offset = match i {
                    3 => above32_old_offset,
                    2 => length32_old_offset,
                    1 => length32_old_offset,
                    0 => 0,
                    _ => {
                        panic!("How? Limit is 0-3")
                    }
                };
                /* effected already has a new base*/
                for j in effected.len() - 1..=0 {
                    effected[j] = unsafe {
                        *((GFS_STRINGS_FIXED_VADDR + offset as u64 + j as u64) as *mut u8)
                    }
                }
            }
        }

        let length16_new_offset = self._lengths[0].len() as u32;
        let length32_new_offset = (self._lengths[0].len() + self._lengths[1].len()) as u32;
        let above32_new_offset =
            (self._lengths[0].len() + self._lengths[1].len() + self._lengths[2].len()) as u32;

        return GfsStringAllocatorReallocationInfo {
            length16_old_offset,
            length32_old_offset,
            above32_old_offset,
            length16_new_offset,
            length32_new_offset,
            above32_new_offset,
        };
    }

    pub fn new(allocator: &mut Allocator) -> Self {
        let length8_bitmap = allocator.alloc_zero(2).unwrap();
        let length16_bitmap = allocator.alloc_zero(2).unwrap();
        let length32_bitmap = allocator.alloc_zero(1).unwrap();
        let length8 =
            unsafe { slice::from_raw_parts_mut(GFS_STRINGS_FIXED_VADDR as *mut u8, 32768) }; // 4096 length 8 entries
        let length16 = unsafe {
            slice::from_raw_parts_mut((GFS_STRINGS_FIXED_VADDR + 32768) as *mut u8, 32768)
        }; // 2048 length 16 entries
        let length32 = unsafe {
            slice::from_raw_parts_mut((GFS_STRINGS_FIXED_VADDR + 32768 + 32768) as *mut u8, 16384)
        }; // 512 length 32 entries
        let length_above64 = unsafe {
            slice::from_raw_parts_mut(
                (GFS_STRINGS_FIXED_VADDR + 32768 + 32768 + 16384 + 20480) as *mut u8,
                28672,
            )
        };

        let memory = MemoryBlock::new(
            28672,
            GFS_STRINGS_FIXED_VADDR + 32768 + 32768 + 16384 + 20480,
        );

        return Self {
            _lengths: [length8, length16, length32, length_above64],
            bitmaps: unsafe {
                [
                    slice::from_raw_parts_mut(length8_bitmap.as_mut_ptr(), 512),
                    slice::from_raw_parts_mut(length16_bitmap.as_mut_ptr(), 256),
                    slice::from_raw_parts_mut(length32_bitmap.as_mut_ptr(), 64),
                ]
            },
            memory: [length8_bitmap, length16_bitmap, length32_bitmap],
            allocator: NodeAllocator::new_fake(allocator, &memory, 28672),
        };
    }
}
