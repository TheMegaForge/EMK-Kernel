use core::{ffi::c_void, ptr::null_mut};

use crate::{
    error,
    hal::{
        ImageAllocation,
        memory::pager::{PAGER_2MIB, Pager},
        print::{Module, simple_kernel_panic},
    },
    info, success,
    utils::{
        Errno,
        memory::{MemoryResult, memcpy, memset_dword, memset_qword},
        traits::{BasicRegion, Region},
    },
    warn,
};

pub const EFI_CONVENTIONAL_MEMORY: u32 = 7;
pub const EFI_UNUSABLE_MEMORY: u32 = 8;
pub const KERNEL_MAXIMUM_ALLOCATABLE_ADDRESS: u64 = 0x800000000; // 32 GiB
pub const KERNEL_MAXIMUM_ALLOCATABLE_PAGES: u32 = 0x800000;
pub const KERNEL_ALLOCATED_PAGES_PER_BITMAP_ENTRY: u32 = 0x20;
pub const KERNEL_NEEDED_BITMAP_ENTRIES: u32 =
    KERNEL_MAXIMUM_ALLOCATABLE_PAGES / KERNEL_ALLOCATED_PAGES_PER_BITMAP_ENTRY;
pub const BITMAP_SIZE_IN_BYTES: u32 = KERNEL_NEEDED_BITMAP_ENTRIES * 4;
pub const BITMAP_SIZE_IN_PAGES: u32 = BITMAP_SIZE_IN_BYTES / 0x1000;

struct BitmapEntry {
    offset: u32,
    bit_offset: u8,
}

impl BitmapEntry {
    pub fn new(offset: u32, bit_offset: u8) -> BitmapEntry {
        return BitmapEntry { offset, bit_offset };
    }

    pub fn from_address(mut address: u64) -> BitmapEntry {
        address /= 0x1000;
        let offset = (address / 32) as u32;
        let bit_offset = (address % 32) as u8;
        return BitmapEntry::new(offset, bit_offset);
    }

    //0000 0000 0000 0000 0000 0000 0000 0000
    //                              ^|------|
    //                              |   ^
    //                self.bit_offset   back bits
    #[allow(dead_code)]
    pub fn get_backbits(&self) -> u32 {
        return self.bit_offset as u32;
    }
    #[allow(dead_code)]
    pub fn get_backbits_ex(&self) -> u32 {
        return self.bit_offset as u32 + 1;
    }

    //0000 0000 0000 0000 0000 0000 0000 0000
    //|---------------------------| ^
    //             ^                |
    //          front bits          self.bit_offset
    pub fn get_frontbits(&self) -> u32 {
        return 31 - self.bit_offset as u32;
    }

    // also counts the bit at self.bit_offset
    pub fn get_frontbits_ex(&self) -> u32 {
        return 32 - self.bit_offset as u32;
    }

    pub fn is_occupied(&self, bitmap: *mut u32, pages: u32) -> bool {
        let begin_mask_width;
        if self.get_frontbits_ex() > pages {
            begin_mask_width = pages;
        } else {
            begin_mask_width = self.get_frontbits_ex();
        }

        let begin_mask = Allocator::make_mask(begin_mask_width as u16) << self.bit_offset;

        if unsafe { *bitmap.add(self.offset as usize) } & begin_mask != begin_mask {
            return false;
        }
        let full_entries = (pages - begin_mask_width) / 32;

        for i in 0..full_entries {
            if unsafe { *bitmap.add((self.offset + i + 1) as usize) } == 0 {
                return false;
            }
        }
        let rem = (pages - begin_mask_width) % 32;
        if rem != 0 {
            let mask = Allocator::make_mask(rem as u16);
            if unsafe { *bitmap.add((self.offset + 1 + full_entries) as usize) } & mask != mask {
                return false;
            }
        }
        return true;
    }

    pub fn set_zero(&self, bitmap: *mut u32, pages: u32) {
        let base = unsafe { bitmap.add(self.offset as usize) };

        let base_mask_width;
        if self.get_frontbits_ex() >= pages {
            base_mask_width = pages;
        } else {
            base_mask_width = self.get_frontbits_ex();
        }
        let base_mask = Allocator::make_mask(base_mask_width as u16) << self.bit_offset;

        unsafe {
            *base = *base & !base_mask;
        }

        let entries_after = (pages - base_mask_width) / 32;

        for i in 0..entries_after {
            unsafe { *base.add(1 + i as usize) = 0 }
        }

        let rem = (pages - base_mask_width) % 32;
        let end = unsafe { base.add(1 + entries_after as usize) };
        if rem != 0 {
            let mask = Allocator::make_mask(rem as u16);
            unsafe {
                *end = *end & !mask;
            }
        }
    }

    pub fn set_one(&self, bitmap: *mut u32, pages: u32) {
        let base = unsafe { bitmap.add(self.offset as usize) };

        let base_mask_width;
        if self.get_frontbits_ex() >= pages {
            base_mask_width = pages;
        } else {
            base_mask_width = self.get_frontbits_ex();
        }
        let base_mask = Allocator::make_mask(base_mask_width as u16) << self.bit_offset;

        unsafe {
            *base = *base | base_mask;
        }

        let entries_after = (pages - base_mask_width) / 32;

        for i in 0..entries_after {
            unsafe { *base.add(1 + i as usize) = 0xFFFFFFFF }
        }

        let rem = (pages - base_mask_width) % 32;
        let end = unsafe { base.add(1 + entries_after as usize) };
        if rem != 0 {
            let mask = Allocator::make_mask(rem as u16);
            unsafe {
                *end = *end | mask;
            }
        }
    }
}

#[repr(C)]
struct EfiMemoryDescriptor {
    descriptor_type: u32,
    physical_start: u64,
    virtual_start: u64,
    number_of_pages: u64,
    attribute: u64,
}

pub struct Allocator {
    bitmap: *mut u32,
    bitmap_entries: u32,
    current_entry: *mut u32,
    addend: u64,
    faked_pages: u32,
}

impl Allocator {
    pub const fn empty() -> Self {
        return Self {
            bitmap: null_mut(),
            bitmap_entries: 0,
            current_entry: null_mut(),
            addend: 0,
            faked_pages: 0,
        };
    }
}

impl Default for Allocator {
    fn default() -> Self {
        return Allocator {
            bitmap: null_mut(),
            bitmap_entries: 0,
            current_entry: null_mut(),
            addend: 0,
            faked_pages: 0,
        };
    }
}
#[derive(Clone, Copy)]
pub struct MemoryBlock {
    pub base: u64,
    pub length: u64,
}

impl MemoryBlock {
    pub const fn empty() -> Self {
        return Self { base: 0, length: 0 };
    }

    pub fn map(
        &self,
        physical_allocator: &mut Allocator,
        pager: &mut Pager,
        vaddr: u64,
        flags: u16,
    ) -> Option<MemoryResult> {
        for i in 0..self.length / 0x1000 {
            match pager.page_4_kb(
                vaddr + i * 0x1000,
                self.base + i * 0x1000,
                flags,
                physical_allocator,
            ) {
                Ok(_) => {}
                Err(e) => return Option::Some(e),
            }
        }
        return Option::None;
    }

    pub fn map_efficient(
        &self,
        physical_allocator: &mut Allocator,
        pager: &mut Pager,
        vaddr: u64,
        flags: u16,
    ) -> Option<MemoryResult> {
        let offset_into_huge_page = (self.base % PAGER_2MIB as u64) / 0x1000;
        let pages_required = 512 - offset_into_huge_page;

        let mut pages_4k = self.length / 0x1000;
        if self.length % 0x1000 != 0 {
            pages_4k += 1;
        }

        if pages_required > pages_4k {
            for i in 0..pages_4k {
                match pager.page_4_kb(
                    vaddr + i * 0x1000,
                    self.base + i * 0x1000,
                    flags,
                    physical_allocator,
                ) {
                    Ok(_) => {}
                    Err(e) => return Option::Some(e),
                }
            }
        } else {
            for i in 0..pages_required {
                match pager.page_4_kb(
                    vaddr + i * 0x1000,
                    self.base + i * 0x1000,
                    flags,
                    physical_allocator,
                ) {
                    Ok(_) => {}
                    Err(e) => return Option::Some(e),
                }
            }
            let pages_2mib = (pages_4k - pages_required) / 512;
            for i in 0..pages_2mib {
                match pager.page_2_mb(
                    vaddr + pages_required * 0x1000 + i * PAGER_2MIB as u64,
                    self.base + pages_required * 0x1000 + i * PAGER_2MIB as u64,
                    flags,
                    physical_allocator,
                ) {
                    Ok(_) => {}
                    Err(e) => return Option::Some(e),
                }
            }
            let rem = pages_4k - (pages_required + pages_2mib * 512);

            for i in 0..rem {
                match pager.page_4_kb(
                    vaddr + pages_required * 0x1000 + pages_2mib * PAGER_2MIB as u64 + i * 0x1000,
                    self.base
                        + pages_required * 0x1000
                        + pages_2mib * PAGER_2MIB as u64
                        + i * 0x1000,
                    flags,
                    physical_allocator,
                ) {
                    Ok(_) => {}
                    Err(e) => return Option::Some(e),
                }
            }
        }
        return Option::None;
    }
}

pub struct PointerBlock<T> {
    pub base: *mut T,
    pub length: u64,
}

impl Default for MemoryBlock {
    fn default() -> Self {
        return MemoryBlock { base: 0, length: 0 };
    }
}

impl<T> Default for PointerBlock<T> {
    fn default() -> Self {
        return Self {
            base: null_mut(),
            length: 0,
        };
    }
}

impl<T> PointerBlock<T> {
    pub fn as_ptr(&self) -> *const T {
        return self.base as *const T;
    }
    pub fn as_mut_ptr(&mut self) -> *mut T {
        return self.base as *mut T;
    }

    pub fn as_ref(&self, offset: u32) -> &T {
        return unsafe { self.as_ptr().add(offset as usize).as_ref().unwrap() };
    }
    pub fn as_mut(&mut self, offset: u32) -> &mut T {
        return unsafe { self.as_mut_ptr().add(offset as usize).as_mut().unwrap() };
    }
}

impl MemoryBlock {
    pub fn as_ptr<T>(&self) -> *const T {
        return self.base as *const T;
    }
    pub fn as_mut_ptr<T>(&self) -> *mut T {
        return self.base as *mut T;
    }

    pub fn as_ref<T>(&self, offset: u32) -> &T {
        return unsafe { self.as_ptr::<T>().add(offset as usize).as_ref().unwrap() };
    }
    pub fn as_mut<T>(&mut self, offset: u32) -> &mut T {
        return unsafe {
            self.as_mut_ptr::<T>()
                .add(offset as usize)
                .as_mut()
                .unwrap()
        };
    }
}

impl Region<u64, u64> for MemoryBlock {
    fn end(&self) -> u64 {
        return self.base + self.length;
    }
    fn get_base(&self) -> u64 {
        return self.base;
    }
    fn get_length(&self) -> u64 {
        return self.length;
    }
    fn new(length: u64, base: u64) -> Self {
        return Self { base, length };
    }

    fn within(&self, other: &MemoryBlock) -> bool {
        return other.base >= self.base && other.end() <= self.end();
    }

    fn offset(&self, offset: u64) -> Option<BasicRegion<u64, u64>> {
        if offset > self.length {
            return Option::None;
        }

        return Option::Some(BasicRegion::<u64, u64>::new(
            self.length - offset,
            self.base + offset,
        ));
    }
}

impl<T> Region<u64, *mut T> for PointerBlock<T> {
    fn new(length: u64, base: *mut T) -> Self {
        return Self { base, length };
    }

    fn end(&self) -> u64 {
        return self.base as u64 + self.length * size_of::<T>() as u64;
    }
    fn get_base(&self) -> *mut T {
        return self.base;
    }
    fn get_length(&self) -> u64 {
        return self.length;
    }

    fn within(&self, other: &Self) -> bool {
        return other.base >= self.base && other.end() <= self.end();
    }
    fn offset(&self, offset: u64) -> Option<BasicRegion<u64, *mut T>> {
        if offset > self.length {
            return Option::None;
        }

        return Option::Some(BasicRegion::<u64, *mut T>::new(
            self.length - offset,
            unsafe { self.base.add(offset as usize) },
        ));
    }
}

impl Allocator {
    pub fn new(
        module: &mut Module,
        descriptor_size: u32,
        descriptor_count: u32,
        memory_descriptor: *mut c_void,
        num_allocations: u32,
        image_allocations: *const c_void,
    ) -> Result<Allocator, Errno> {
        let mut bitmap: *mut u32 = null_mut();
        for i in 0..descriptor_count {
            let memory_descriptor = unsafe {
                memory_descriptor.add((descriptor_size * i) as usize) as *mut EfiMemoryDescriptor
            };

            if unsafe { (*memory_descriptor).number_of_pages } >= BITMAP_SIZE_IN_PAGES as u64
                && unsafe { (*memory_descriptor).descriptor_type == EFI_CONVENTIONAL_MEMORY }
            {
                bitmap = unsafe { (*memory_descriptor).physical_start as *mut u32 };
                unsafe { (*memory_descriptor).number_of_pages -= BITMAP_SIZE_IN_PAGES as u64 };
                unsafe { (*memory_descriptor).physical_start += BITMAP_SIZE_IN_BYTES as u64 };
                if (unsafe { (*memory_descriptor).number_of_pages } == 0) {
                    unsafe { (*memory_descriptor).descriptor_type = EFI_UNUSABLE_MEMORY }
                }
                info!(module, "Found bitmap for allocator at descriptor {}\n", i);
                break;
            }
        }
        if bitmap.is_null() {
            error!(module, "Could not find bitmap for allocator\n");
            return Err(Errno::ENOBUFS);
        };

        unsafe {
            memset_dword(bitmap as *mut c_void, 0xFFFFFFFF, BITMAP_SIZE_IN_BYTES / 4);
        }

        for i in 0..descriptor_count {
            let memory_descriptor = unsafe {
                memory_descriptor.add((descriptor_size * i) as usize) as *mut EfiMemoryDescriptor
            };

            if unsafe { (*memory_descriptor).descriptor_type == EFI_CONVENTIONAL_MEMORY } {
                if unsafe { (*memory_descriptor).physical_start }
                    + unsafe { (*memory_descriptor).number_of_pages } * 0x1000
                    > KERNEL_MAXIMUM_ALLOCATABLE_ADDRESS
                {
                    warn!(module, "Skipping conventional memory descriptor {}\n", i);
                    break;
                }
                let entry =
                    BitmapEntry::from_address(unsafe { (*memory_descriptor).physical_start });
                entry.set_zero(bitmap, unsafe { (*memory_descriptor).number_of_pages }
                    as u32);
            }
        }

        for i in 0..num_allocations {
            let image_allocation =
                unsafe { (image_allocations as *const ImageAllocation).add(i as usize) };
            if unsafe { (*image_allocation).allocated } == 0 {
                continue;
            }
            let entry = BitmapEntry::from_address(unsafe { (*image_allocation).allocated });
            entry.set_one(bitmap, unsafe { (*image_allocation).size / 0x1000 } as u32);
        }

        // Memory for core bootstrap
        let entry = BitmapEntry::from_address(0x8000);
        entry.set_one(bitmap, 4);

        success!(module, "initialized allocator\n");
        return Ok(Allocator {
            bitmap: bitmap,
            bitmap_entries: KERNEL_NEEDED_BITMAP_ENTRIES,
            current_entry: bitmap,
            addend: 0,
            faked_pages: 0,
        });
    }

    pub fn lowest_address(&self) -> u64 {
        return self.addend;
    }

    pub fn subdivide(&mut self, num_pages: u16) -> Allocator {
        let mut num_entries_needed = num_pages / 32;
        if num_pages % 32 != 0 {
            num_entries_needed += 1;
        }
        let mut bitmap_pages_needed = (num_entries_needed * 4) / 0x1000;
        if (num_entries_needed * 4) % 0x1000 != 0 {
            bitmap_pages_needed += 1;
        }
        let bitmap = match self.alloc_zero(bitmap_pages_needed) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => simple_kernel_panic("Allocator/subdivide", "Could not allocate bitmap\n"),
        };
        // Allocates on the current bitmap, so &self won´t allocate memory which the subdivided allocator can allocate
        let addend = match self.alloc(num_pages) {
            Ok(mb) => mb.get_base(),
            Err(_e) => simple_kernel_panic(
                "Allocator/subdivide",
                "Could not allocate addend memory base\n",
            ),
        };

        return Allocator {
            bitmap,
            bitmap_entries: num_entries_needed as u32,
            current_entry: bitmap,
            addend,
            faked_pages: 0,
        };
    }
    pub fn new_fake(&mut self, num_pages: u16, virtual_address: u64) -> Allocator {
        let mut num_entries_needed = num_pages / 32;
        if num_pages % 32 != 0 {
            num_entries_needed += 1;
        }
        let mut bitmap_pages_needed = (num_entries_needed * 4) / 0x1000;
        if (num_entries_needed * 4) % 0x1000 != 0 {
            bitmap_pages_needed += 1;
        }
        let bitmap = match self.alloc_zero(bitmap_pages_needed) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => simple_kernel_panic("Allocator/subdivide", "Could not allocate bitmap\n"),
        };

        return Allocator {
            bitmap,
            bitmap_entries: num_entries_needed as u32,
            current_entry: bitmap,
            addend: virtual_address,
            faked_pages: num_pages as u32,
        };
    }

    pub fn expand(&mut self, super_allocator: &mut Allocator, more_pages: u16) {
        if self.faked_pages == 0 {
            return;
        }
        let num_pages = self.faked_pages + more_pages as u32;
        let mut num_entries_needed = num_pages / 32;
        if num_pages % 32 != 0 {
            num_entries_needed += 1;
        }
        let mut bitmap_pages_needed = (num_entries_needed * 4) / 0x1000;
        if (num_entries_needed * 4) % 0x1000 != 0 {
            bitmap_pages_needed += 1;
        }
        let bitmap = match self.alloc_zero(bitmap_pages_needed as u16) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => simple_kernel_panic("Allocator/subdivide", "Could not allocate bitmap\n"),
        };
        unsafe {
            memcpy(
                bitmap,
                self.bitmap as *const c_void,
                self.bitmap_entries * 4,
            );
        }

        let mb = MemoryBlock::new(
            ((self.bitmap_entries * 4) / 0x1000) as u64,
            self.bitmap as u64,
        );
        super_allocator.free(&mb).unwrap();
        let offset = unsafe { self.current_entry.offset_from(self.bitmap) } as usize;
        self.current_entry = unsafe { (bitmap as *mut u32).add(offset) };
        self.bitmap = bitmap as *mut u32;
        self.bitmap_entries = num_entries_needed;
    }

    fn alloc_ex(&mut self, pages: u16) -> Result<MemoryBlock, MemoryResult> {
        let (entry_offset, entry, entry_bit_offset) = match self.next_free_entry_ex(pages) {
            Ok(off) => (off.0, unsafe { self.bitmap.add(off.0 as usize) }, off.1),
            Err(e) => return Result::Err(e),
        };
        let first_mask = Allocator::make_mask(32 - entry_bit_offset as u16);

        unsafe {
            (*entry) |= first_mask << entry_bit_offset;
        }

        let full_entries = (pages - (32 - entry_bit_offset) as u16) / 32;

        for i in 0..full_entries {
            unsafe {
                *(entry.add(1 + i as usize)) = 0xFFFFFFFF;
            }
        }

        if (pages - (32 - entry_bit_offset) as u16) % 32 != 0 {
            let end_mask = Allocator::make_mask((pages - (32 - entry_bit_offset) as u16) % 32);
            unsafe {
                *entry.add((1 + full_entries) as usize) |= end_mask;
            }
        }

        let base = self.addend + (entry_offset as u64 * 0x20000) + entry_bit_offset as u64 * 0x1000;
        let length = pages as u64 * 0x1000;

        return Result::Ok(MemoryBlock::new(length, base));
    }

    fn next_free_entry(&mut self, pages: u16) -> Result<(u8, u32), MemoryResult> {
        let mut ptr = self.bitmap;
        let mut inc = 0;
        while ptr <= unsafe { self.bitmap.add(self.bitmap_entries as usize) } {
            match self.is_entry_valid(ptr, pages) {
                Some(bit_off) => {
                    return Result::Ok((bit_off, inc));
                }
                None => {}
            }
            inc += 1;
            ptr = unsafe { ptr.offset(1) };
        }

        return Result::Err(MemoryResult::Nospace);
    }

    fn next_free_entry_ex(&mut self, pages: u16) -> Result<(u32, u8), MemoryResult> {
        let mut inc = 0;
        let mut ptr = self.bitmap;
        while ptr <= unsafe { self.bitmap.add(self.bitmap_entries as usize) } {
            if unsafe { *ptr } & (1 << 31) == 0 {
                let mut mask = 0;
                let mut bits_avail: u16 = 32;

                let mut i = 32;

                while i != 0i16 {
                    mask |= 1 << (i - 1);
                    i -= 1;
                    if unsafe { *ptr } & mask != 0 {
                        bits_avail = 32 - (i + 1) as u16;
                        break;
                    }
                }

                let full_entries_required = (pages - bits_avail) / 32;

                let mut failed = false;

                for i in 0..full_entries_required {
                    unsafe {
                        if *ptr.add(1 + i as usize) != 0 {
                            failed = true;
                            break;
                        }
                    }
                }
                if failed {
                    inc += 1;
                    ptr = unsafe { ptr.offset(1) };
                    continue;
                }

                let end_mask = Allocator::make_mask((pages - bits_avail) % 32);
                if unsafe { *ptr.add(1 + full_entries_required as usize) } & end_mask != 0 {
                    inc += 1;
                    ptr = unsafe { ptr.offset(1) };
                    continue;
                }
                return Result::Ok((inc, 32 - bits_avail as u8));
            }
            inc += 1;
            ptr = unsafe { ptr.offset(1) };
        }

        return Result::Err(MemoryResult::Nospace);
    }

    fn make_mask(pages: u16) -> u32 {
        let mut bit: u64 = 1 << pages;
        bit -= 1;

        return bit as u32;
    }

    fn is_entry_valid(&self, ptr: *mut u32, pages: u16) -> Option<u8> {
        let mask = Allocator::make_mask(pages);
        for i in 0..=(32 - pages as u8) {
            if (unsafe { *ptr } & (mask << i)) == 0 {
                return Option::Some(i);
            }
        }

        return Option::None;
    }

    fn get_current_entry_base(&self) -> u64 {
        let entries_offset = (self.current_entry as u64 - self.bitmap as u64) / 4;
        return entries_offset * (KERNEL_ALLOCATED_PAGES_PER_BITMAP_ENTRY as u64) * 0x1000;
    }

    pub fn alloc(&mut self, pages: u16) -> Result<MemoryBlock, MemoryResult> {
        if pages == 0 {
            return Result::Err(MemoryResult::AllocationError);
        }
        if 32 >= pages {
            let bit_off;
            match self.is_entry_valid(self.current_entry, pages) {
                Some(bit_offset) => bit_off = bit_offset,
                None => {
                    self.current_entry = match self.next_free_entry(pages) {
                        Ok((bit_offset, entry_offset)) => {
                            bit_off = bit_offset;
                            unsafe { self.bitmap.add(entry_offset as usize) }
                        }
                        Err(e) => return Result::Err(e),
                    };
                }
            }
            let mask = Allocator::make_mask(pages);
            unsafe {
                (*self.current_entry) |= mask << bit_off;
            }

            let length = (pages as u64) * 0x1000;
            let base = self.addend + self.get_current_entry_base() + (bit_off as u64) * 0x1000;

            return Result::Ok(MemoryBlock::new(length, base));
        } else {
            return self.alloc_ex(pages);
        }
    }

    pub fn alloc_zero(&mut self, pages: u16) -> Result<MemoryBlock, MemoryResult> {
        match self.alloc(pages) {
            Ok(mb) => {
                unsafe {
                    memset_qword(mb.as_mut_ptr(), 0, (mb.get_length() / 8) as u32);
                }
                return Result::Ok(mb);
            }
            Err(e) => return Result::Err(e),
        }
    }

    pub fn free(&mut self, memblock: &MemoryBlock) -> Result<(), MemoryResult> {
        if memblock.length & 0xFFF != 0 {
            return Result::Err(MemoryResult::InvalidLength);
        }
        if memblock.base < self.addend
            || memblock.base & 0xFFF != 0
            || memblock.end() > KERNEL_MAXIMUM_ALLOCATABLE_ADDRESS
        {
            return Result::Err(MemoryResult::InvalidAddress);
        }

        let entry = BitmapEntry::from_address(memblock.base - self.addend);
        if !entry.is_occupied(self.bitmap, memblock.length as u32 / 0x1000) {
            return Result::Err(MemoryResult::InvalidBlock);
        }
        entry.set_zero(self.bitmap, (memblock.length / 0x1000) as u32);

        return Result::Ok(());
    }
}

pub struct VirtualAllocator {
    pub physical: MemoryBlock,
    pub allocator: Allocator,
}

impl VirtualAllocator {
    pub fn new(memory: MemoryBlock, allocator: Allocator) -> Self {
        return Self {
            physical: memory,
            allocator,
        };
    }
}
