use crate::{
    aml::definitions::TermArgInt::Add,
    fixed_vaddrs::{FIXED_KERNEL_SPACE_MEMORY_VADDR, FIXED_PROCESSOR_VIRTUAL_ADDRESS},
    hal::{
        ImageAllocation,
        memory::allocator::{Allocator, MemoryBlock, VirtualAllocator},
        print::{Module, get_framebuffer, simple_kernel_panic},
    },
    multithreading::processors::Processor,
    processes::ExecutionMode,
    success,
    utils::{
        intrin::{set_cr3, wrmsr},
        memory::MemoryResult,
        traits::Region,
    },
};
use core::{ffi::c_void, ptr::null_mut};

pub struct Page {
    physical: u64,
    virt: u64,
}

pub fn page_align<T>(ptr: *const T) -> u64 {
    return ptr as u64 & !0xFFF;
}

impl Page {
    pub fn new(physical: u64, virt: u64) -> Page {
        return Page { physical, virt };
    }

    pub fn get_physical(&self) -> u64 {
        return self.physical;
    }

    pub fn get_virt(&self) -> u64 {
        return self.virt;
    }
}

struct Address {
    pub pdp: u16,
    pub pd: u16,
    pub pt: u16,
    pub page: u16,
}

enum PointerTableType {
    PDPT,
    #[allow(unused)]
    PDT,
    #[allow(unused)]
    PT,
}

struct PointerTable {
    location: *mut u64,
    flags: u16,
    address: u64,
    address_increment: u32,
}

//TODO: Implement proper PointerTable!

impl PointerTable {
    pub fn new(ptt: PointerTableType, base: *mut u64, index: u16, flags: u16) -> PointerTable {
        let location = unsafe { base.add(index as usize) };
        let increment = match ptt {
            PointerTableType::PDPT => 0,
            PointerTableType::PDT => PAGER_1GIB,
            PointerTableType::PT => PAGER_2MIB,
        };
        let address = unsafe { *location & !0xFFF };
        return PointerTable {
            location,
            flags,
            address,
            address_increment: increment,
        };
    }

    pub fn read_flags(&self) -> u16 {
        return unsafe { (*self.location) as u16 } & 0xFFF;
    }

    pub fn page_size_active(&self) -> bool {
        return unsafe { *self.location } & PAGER_PAGE_SIZE as u64 != 0;
    }

    pub fn instantiate(&self, index: u16) -> Option<PointerTable> {
        if unsafe { *self.location } & PAGER_PAGE_SIZE as u64 != 0 {
            return Option::None;
        }
        let increment = match self.address_increment {
            0 => PAGER_1GIB,
            PAGER_1GIB => PAGER_2MIB,
            _ => return Option::None,
        };
        let location = unsafe { (self.address as *mut u64).add(index as usize) };
        let address = unsafe { *location & !0xFFF };
        return Option::Some(PointerTable {
            location,
            flags: self.flags,
            address,
            address_increment: increment,
        });
    }

    pub fn unfold(&mut self, allocator: &mut Allocator) -> Option<MemoryResult> {
        if !self.page_size_active() {
            return Option::Some(MemoryResult::InvalidActivateFlags);
        }
        let ptr: *mut u64 = match allocator.alloc_zero(1) {
            Ok(mb) => mb.get_base() as *mut u64,
            Err(e) => return Option::Some(e),
        };
        self.flags = unsafe { *self.location } as u16 & 0xFFF;
        self.flags &= !PAGER_PAGE_SIZE;

        for i in 0..512 {
            unsafe {
                *ptr.add(i) = self.address + i as u64 * 0x1000 | self.flags as u64;
            }
        }

        self.address = ptr as u64;
        self.flags |= PAGER_STAB_ALLOCATED;
        unsafe { *self.location = self.address | self.flags as u64 }

        return Option::None;
    }

    //If page size => address will set the 2MB physical.
    pub fn set_new_address(&mut self, address: u64) {
        unsafe {
            *self.location = address | self.flags as u64;
        }
        self.address = address;
    }

    pub fn allocate_if_necessary(
        &mut self,
        allocator: &mut Allocator,
    ) -> Result<bool, MemoryResult> {
        if unsafe { *self.location } == 0 && self.flags & PAGER_PAGE_SIZE == 0 {
            let base = match allocator.alloc_zero(1) {
                Ok(mb) => mb.get_base(),
                Err(e) => return Result::Err(e),
            };
            unsafe {
                self.flags |= PAGER_STAB_ALLOCATED;
                *self.location = base | self.flags as u64;
                self.address = base;
            };
            return Result::Ok(true);
        }
        return Result::Ok(false);
    }

    pub fn is_allocated(&self) -> bool {
        return unsafe { *self.location } & PAGER_STAB_ALLOCATED as u64 != 0;
    }

    pub fn is_present(&self) -> bool {
        return unsafe { *self.location } != 0;
    }

    pub fn deallocate(&mut self, allocator: &mut Allocator) -> Option<MemoryResult> {
        let mb = MemoryBlock::new(0x1000, self.address);
        match allocator.free(&mb) {
            Ok(_) => {
                self.flags ^= PAGER_STAB_ALLOCATED;
                unsafe {
                    *self.location ^= PAGER_STAB_ALLOCATED as u64;
                }
                return Option::None;
            }
            Err(e) => return Option::Some(e),
        }
    }
}

pub struct Pager {
    cr3: *mut u64,
}

pub const PAGER_PAGE_SIZE: u16 = 1 << 7;
pub const PAGER_STAB_ALLOCATED: u16 = 1 << 8;
pub const PAGER_2MIB: u32 = 0x200000;
pub const PAGER_1GIB: u32 = 0x40000000;
pub const PAGER_512GIB: u64 = 0x8000000000;
pub const PAGER_US: u16 = 1 << 2;
pub const PAGER_RW: u16 = 1 << 1;
pub const PAGER_PRESENT: u16 = 1;
pub const PAGER_PCD: u16 = 1 << 4;
pub const PAGER_PAT: u16 = 1 << 12;
pub const PAGER_PAT2: u16 = 1 << 7; // for page table entry
pub const IA32_PAT_MSR: u64 = 0x277;
pub const PAT_RESET_VALUE: u64 = 0x0007040600070406;
#[unsafe(link_section = ".host_core")]
pub static mut HOST_CORE_PAGER: Pager = Pager { cr3: null_mut() };

impl Pager {
    pub fn get_cr3(&self) -> *const u64 {
        return self.cr3;
    }

    pub fn page_kernelspace_memory(
        &mut self,
        physical_allocator: &mut Allocator,
        virtual_allocator: &mut VirtualAllocator,
    ) -> Option<MemoryResult> {
        for i in 0..virtual_allocator.physical.length / 0x1000 {
            match self.page_4_kb(
                FIXED_KERNEL_SPACE_MEMORY_VADDR + i * 0x1000,
                virtual_allocator.physical.base + i * 0x1000,
                PAGER_RW | PAGER_PRESENT,
                physical_allocator,
            ) {
                Ok(_) => {}
                Err(e) => return Option::Some(e),
            };
        }
        return Option::None;
    }

    pub fn page_kernel(
        &mut self,
        allocator: &mut Allocator,
        num_allocations: u32,
        image_allocations: *const c_void,
    ) -> Option<MemoryResult> {
        for i in 0..num_allocations {
            let image_allocation =
                unsafe { (image_allocations as *const ImageAllocation).add(i as usize) };

            let section = unsafe { &*image_allocation };

            if section.allocated == 0 {
                continue;
            }

            let mb = MemoryBlock::new(section.size, section.allocated);

            match mb.map(allocator, self, section.phdr, PAGER_RW | PAGER_PRESENT) {
                Some(e) => return Option::Some(e),
                None => {}
            }
        }
        return Option::None;
    }

    pub fn page_stack(
        &mut self,
        allocator: &mut Allocator,
        stack: *const c_void,
    ) -> Option<MemoryResult> {
        for i in 0..32 {
            match self.page_4_kb(
                stack as u64 + i * 0x1000,
                stack as u64 + i * 0x1000,
                PAGER_RW | PAGER_PRESENT,
                allocator,
            ) {
                Ok(_) => {}
                Err(e) => return Option::Some(e),
            }
        }
        return Option::None;
    }

    pub fn page_framebuffer(&mut self, allocator: &mut Allocator) -> Option<MemoryResult> {
        let framebuffer_base = unsafe { (*get_framebuffer()).get_base() };
        for i in 0..unsafe { (*get_framebuffer()).get_size() } / 0x1000 {
            match self.page_4_kb(
                framebuffer_base + (i * 0x1000) as u64,
                framebuffer_base + (i * 0x1000) as u64,
                PAGER_PRESENT | PAGER_RW | PAGER_PAT,
                allocator,
            ) {
                Ok(_) => {}
                Err(e) => return Option::Some(e),
            }
        }
        return Option::None;
    }

    pub fn page_general(&mut self, allocator: &mut Allocator) -> Option<MemoryResult> {
        let loops = 0x80000000 / PAGER_2MIB as u64;
        for i in 0..loops {
            match self.page_2_mb(
                i * PAGER_2MIB as u64,
                i * PAGER_2MIB as u64,
                PAGER_PRESENT | PAGER_RW,
                allocator,
            ) {
                Ok(_) => {}
                Err(e) => return Option::Some(e),
            }
        }
        return Option::None;
    }

    /*Info: Also initializes FIXED_PROCESSOR_VIRTUAL_ADDRESS for host core*/
    pub fn host_core(
        physical_allocator: &mut Allocator,
        kernel_allocator: &mut Allocator,
        virtual_allocator: &mut VirtualAllocator,
        num_allocations: u32,
        image_allocations: *const c_void,
        stack: *const c_void,
        cr3: *mut u64,
    ) -> Result<Pager, MemoryResult> {
        let mut module = Module::new("Pager");

        let mut ret = Pager::new(cr3);

        let page_option = ret.page_general(physical_allocator);

        match page_option {
            Some(_e) => simple_kernel_panic(module.name(), "Could not page general\n"),
            None => {}
        }

        match ret.page_stack(physical_allocator, stack) {
            Some(_e) => {
                simple_kernel_panic(module.name(), "Could not page stack\n");
            }
            None => {}
        }

        match ret.page_kernelspace_memory(physical_allocator, virtual_allocator) {
            Some(_e) => simple_kernel_panic(module.name(), "Could not page kernelspace memory\n"),
            None => {}
        };

        match ret.page_kernel(physical_allocator, num_allocations, image_allocations) {
            Some(_e) => simple_kernel_panic(module.name(), "Could not page kernel\n"),
            None => {}
        }

        unsafe {
            wrmsr(IA32_PAT_MSR, PAT_RESET_VALUE);
        }

        match ret.page_framebuffer(physical_allocator) {
            Some(_e) => simple_kernel_panic(module.name(), "Could not page framebuffer\n"),
            None => {}
        }

        let host_core_processor: *mut Processor = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => {
                simple_kernel_panic(module.name(), "Could not allocate host core processor\n");
            }
        };
        match ret.page_4_kb(
            FIXED_PROCESSOR_VIRTUAL_ADDRESS,
            host_core_processor as u64,
            PAGER_PRESENT | PAGER_RW,
            physical_allocator,
        ) {
            Ok(_) => {}
            Err(_e) => simple_kernel_panic(module.name(), "Could not page host core processor\n"),
        }
        ret.update_cr3();
        unsafe {
            (*host_core_processor) = Processor::new(
                virtual_allocator,
                physical_allocator,
                kernel_allocator,
                0,
                host_core_processor as u64,
                0,
                0,
            );
        }
        success!(module, "Initialized for Core 0\n");
        return Result::Ok(ret);
    }

    pub fn update_cr3(&self) {
        unsafe { set_cr3(self.cr3) };
    }

    pub fn new(cr3: *mut u64) -> Pager {
        return Pager { cr3 };
    }
    #[allow(unused)]
    fn into_address(pdp: u16, pd: u16, pt: u16, page: u16) -> u64 {
        return page as u64 * 0x1000
            + pt as u64 * PAGER_2MIB as u64
            + pd as u64 * PAGER_1GIB as u64
            + pdp as u64 * PAGER_512GIB;
    }

    fn translate(mut virt: u64) -> Address {
        virt &= !0xFFF;
        virt >>= 12;
        let page = (virt & 0x1ff) as u16;
        virt >>= 9;
        let pt = (virt & 0x1ff) as u16;
        virt >>= 9;
        let pd = (virt & 0x1ff) as u16;
        virt >>= 9;
        let pdp = (virt & 0x1ff) as u16;
        return Address { pdp, pd, pt, page };
    }

    fn get_pd_allocated(
        &mut self,
        address: &Address,
        flags: u16,
        allocator: &mut Allocator,
    ) -> Result<PointerTable, MemoryResult> {
        let mut pdpt = PointerTable::new(PointerTableType::PDPT, self.cr3, address.pdp, flags);
        match pdpt.allocate_if_necessary(allocator) {
            Ok(_) => {}
            Err(e) => return Result::Err(e),
        };
        if flags & PAGER_RW != 0 && pdpt.read_flags() & PAGER_RW == 0 {
            unsafe { *(pdpt.location) |= PAGER_RW as u64 };
        }
        let mut pd = match pdpt.instantiate(address.pd) {
            Some(pd) => pd,
            None => return Result::Err(MemoryResult::PagingError),
        };
        if pd.page_size_active() {
            return Result::Err(MemoryResult::InvalidActivateFlags);
        }
        match pd.allocate_if_necessary(allocator) {
            Ok(_) => {}
            Err(e) => return Result::Err(e),
        };
        if flags & PAGER_RW != 0 && pd.read_flags() & PAGER_RW == 0 {
            unsafe { *(pd.location) |= PAGER_RW as u64 };
        }
        return Result::Ok(pd);
    }

    fn get_pd_unallocated(
        &self,
        address: &Address,
        flags: u16,
    ) -> Result<PointerTable, MemoryResult> {
        let pdpt = PointerTable::new(PointerTableType::PDPT, self.cr3, address.pdp, flags);
        if !pdpt.is_present() {
            return Result::Err(MemoryResult::PagingError);
        }
        let pd = match pdpt.instantiate(address.pd) {
            Some(pd) => pd,
            None => return Result::Err(MemoryResult::PagingError),
        };
        if !pd.is_present() {
            return Result::Err(MemoryResult::PagingError);
        }
        if pd.page_size_active() {
            return Result::Err(MemoryResult::InvalidActivateFlags);
        }
        return Result::Ok(pd);
    }
    /**
     * returns the virtual memory address.
     * PT´s which aren´t complete zero are ignored
     */
    pub fn next_free_virtual_2mb(&self) -> Option<u64> {
        for pdpi in 0..512 {
            let pdpt = PointerTable::new(PointerTableType::PDPT, self.cr3, pdpi, unsafe {
                (*self.cr3.add(pdpi as usize)) & 0xFFF
            }
                as u16);

            if pdpt.flags & PAGER_PRESENT == 0 {
                continue;
            }

            for pdi in 0..512 {
                let pdt = pdpt.instantiate(pdi).unwrap();

                if pdt.read_flags() & PAGER_PRESENT == 0 {
                    continue;
                }

                for pti in 0..512 {
                    let pt = pdt.instantiate(pti).unwrap();
                    if (unsafe { *pt.location }) == 0 {
                        return Option::Some(Pager::into_address(pdpi, pdi, pti, 0));
                    }
                }
            }
        }

        return Option::None;
    }

    /**
     * Checks if self uses a virtual address, which is allready present in other
     */
    pub fn occupation_check(&self, other: &Pager) -> bool {
        for pdpi in 0..512 {
            let self_pdpt = PointerTable::new(PointerTableType::PDPT, self.cr3, pdpi, unsafe {
                (*self.cr3.add(pdpi as usize)) & 0xFFF
            }
                as u16);
            let other_pdpt = PointerTable::new(PointerTableType::PDPT, other.cr3, pdpi, unsafe {
                (*other.cr3.add(pdpi as usize)) & 0xFFF
            }
                as u16);

            if other_pdpt.flags & PAGER_PRESENT == 0 || self_pdpt.flags & PAGER_PRESENT == 0 {
                /*
                 * if a pdpt is not present, all memory inside of it is deactivated, thus not occupying memory
                 */
                continue;
            }

            for pdi in 0..512 {
                let self_pdt = self_pdpt.instantiate(pdi).unwrap();
                let other_pdt = other_pdpt.instantiate(pdi).unwrap();

                if other_pdt.read_flags() & PAGER_PRESENT == 0
                    || self_pdt.read_flags() & PAGER_PRESENT == 0
                {
                    /* Same reason as for the pdpt*/
                    continue;
                }

                for pti in 0..512 {
                    let self_pt = self_pdt.instantiate(pti).unwrap();
                    let other_pt = self_pdt.instantiate(pti).unwrap();

                    if other_pt.read_flags() & PAGER_PRESENT == 0
                        || self_pt.read_flags() & PAGER_PRESENT == 0
                    {
                        continue;
                    }

                    if (other_pt.read_flags() & PAGER_PAGE_SIZE != 0
                        || self_pt.read_flags() & PAGER_PAGE_SIZE != 0)
                        && (other_pt.read_flags() & PAGER_PRESENT != 0
                            || self_pt.read_flags() & PAGER_PRESENT != 0)
                    {
                        /*
                         * PAGER_PAGE_SIZE indicates that the PT occupies 2mb
                         * Which means that if the opposite pager (self <=> other) also has a valid pt, memory is being occupied by both pagers
                         */
                        return true;
                    }

                    for pi in 0..512 {
                        let self_page = unsafe { *((self_pt.address as *const u64).add(pi)) };
                        let other_page = unsafe { *((other_pt.address as *const u64).add(pi)) };

                        if self_page & PAGER_PRESENT as u64 != 0
                            && other_page & PAGER_PRESENT as u64 != 0
                        {
                            /* pages, which are for the same memory address are present, thus occupying allready occupied memory*/
                            return true;
                        }
                    }
                }
            }
        }

        return false;
    }

    /**
     * copies the mapping of other into self, with an offset (base)
     * e.g
     *  If other maps vaddr 0x1000 to paddr 0x4000 and if base = 0x20000 then self will map vaddr 0x21000 to paddr 0x4000
     *  occupation_check() has to be called before this.
     */
    pub fn bind_other(
        &mut self,
        base: u64,
        other: &Pager,
        physical_allocator: &mut Allocator,
    ) -> Option<MemoryResult> {
        for pdpi in 0..512 {
            let mut pdpt = PointerTable::new(PointerTableType::PDPT, other.cr3, pdpi, unsafe {
                (*other.cr3.add(pdpi as usize)) & 0xFFF
            }
                as u16);

            if pdpt.flags & PAGER_PRESENT == 0 {
                continue;
            }

            for pdi in 0..512 {
                let pdt = pdpt.instantiate(pdi).unwrap();
                if pdt.read_flags() & PAGER_PRESENT == 0 {
                    continue;
                }

                for pti in 0..512 {
                    let pt = pdt.instantiate(pti).unwrap();

                    if pt.read_flags() & PAGER_PRESENT == 0 {
                        continue;
                    }

                    if pt.page_size_active() {
                        let vaddr = base + Pager::into_address(pdpi, pdi, pti, 0);
                        if vaddr % PAGER_2MIB as u64 == 0 {
                            match self.page_2_mb(
                                vaddr,
                                pt.address,
                                pt.read_flags(),
                                physical_allocator,
                            ) {
                                Ok(_) => {}
                                Err(e) => return Option::Some(e),
                            }
                        } else {
                            let mb: MemoryBlock = MemoryBlock::new(PAGER_2MIB as u64, pt.address);
                            match mb.map_efficient(physical_allocator, self, vaddr, pt.read_flags())
                            {
                                Some(e) => return Option::Some(e),
                                None => {}
                            }
                        }
                    } else {
                        for pi in 0..512 {
                            let raw = unsafe { *(pt.address as *const u64).add(pi as usize) };
                            let paddr = raw & !0xFFF;

                            if raw & PAGER_PRESENT as u64 == 0 {
                                continue;
                            }

                            let vaddr = Pager::into_address(pdpi, pdi, pti, pi);
                            match self.page_4_kb(
                                base + vaddr,
                                paddr,
                                (raw & 0xFFF) as u16,
                                physical_allocator,
                            ) {
                                Ok(_) => {}
                                Err(e) => return Option::Some(e),
                            }
                        }
                    }
                }
            }
        }

        return Option::None;
    }

    /*
     * deallocates used memory
     */
    pub fn release(&mut self, allocator: &mut Allocator) -> Option<MemoryResult> {
        for pdpi in 0..512 {
            let mut pdpt = PointerTable::new(PointerTableType::PDPT, self.cr3, pdpi, unsafe {
                (*self.cr3.add(pdpi as usize)) & 0xFFF
            }
                as u16);
            if pdpt.flags & PAGER_STAB_ALLOCATED == 0 {
                continue;
            }

            for pdi in 0..512 {
                let mut pdt = pdpt.instantiate(pdi).unwrap();
                if pdt.read_flags() & PAGER_STAB_ALLOCATED == 0 {
                    continue;
                }

                for pti in 0..512 {
                    let mut pt = pdt.instantiate(pti).unwrap();
                    if pt.is_allocated() {
                        match pt.deallocate(allocator) {
                            Some(e) => return Option::Some(e),
                            None => {}
                        }
                    }
                }
                match pdt.deallocate(allocator) {
                    Some(e) => return Option::Some(e),
                    None => {}
                }
            }
            match pdpt.deallocate(allocator) {
                Some(e) => return Option::Some(e),
                None => {}
            }
        }
        match allocator.free(&MemoryBlock::new(0x1000, self.cr3 as u64)) {
            Ok(_) => return Option::None,
            Err(e) => return Option::Some(e),
        }
    }

    pub fn page_4_kb(
        &mut self,
        virt: u64,
        physical: u64,
        mut flags: u16,
        allocator: &mut Allocator,
    ) -> Result<Page, MemoryResult> {
        if virt & 0xFFF != 0 || physical & 0xFFF != 0 {
            return Result::Err(MemoryResult::InvalidAddress);
        }
        let pat_used = (flags & PAGER_PAT) != 0;
        if pat_used {
            flags ^= PAGER_PAT;
        }
        if flags & 0xF0A0 != 0 {
            return Result::Err(MemoryResult::InvalidFlags);
        }
        let address = Pager::translate(virt);
        let pd = match self.get_pd_allocated(&address, flags, allocator) {
            Ok(pd) => pd,
            Err(e) => return Result::Err(e),
        };
        let mut pt = match pd.instantiate(address.pt) {
            Some(pt) => pt,
            None => return Result::Err(MemoryResult::PagingError),
        };
        if pt.page_size_active() {
            match pt.unfold(allocator) {
                Some(e) => return Result::Err(e),
                None => {}
            };
            if pat_used {
                flags ^= PAGER_PAT2;
            }
            let page_ptr = unsafe { (pt.address as *mut u64).add(address.page as usize) };
            unsafe {
                *page_ptr = physical | flags as u64;
            }
        } else {
            if pat_used {
                flags ^= PAGER_PAT2;
            }
            match pt.allocate_if_necessary(allocator) {
                Ok(_) => {}
                Err(e) => return Err(e),
            }
            let page_ptr = unsafe { (pt.address as *mut u64).add(address.page as usize) };
            unsafe {
                *page_ptr = physical | flags as u64;
            }
        }
        //TODO: invalidate page!
        return Result::Ok(Page::new(physical, virt));
    }

    pub fn unpage_4k(&mut self, virt: u64) -> Option<MemoryResult> {
        if virt & 0xFFF != 0 {
            return Option::Some(MemoryResult::InvalidAddress);
        }

        let address = Pager::translate(virt);
        let pd = match self.get_pd_unallocated(&address, PAGER_PRESENT) {
            Ok(pd) => pd,
            Err(e) => return Option::Some(e),
        };
        let pt = match pd.instantiate(address.pt) {
            Some(pt) => pt,
            None => return Option::Some(MemoryResult::PagingError),
        };
        if pt.page_size_active() {
            return Option::Some(MemoryResult::InvalidActivateFlags);
        } else {
            unsafe {
                *(pt.address as *mut u64).add(address.page as usize) = 0;
            }
        }
        return Option::None;
    }

    pub fn page_2_mb(
        &mut self,
        virt: u64,
        physical: u64,
        flags: u16,
        allocator: &mut Allocator,
    ) -> Result<Page, MemoryResult> {
        if virt & 0xFFFFF != 0 || physical & 0xFFFFF != 0 {
            return Result::Err(MemoryResult::InvalidAddress);
        }
        if flags & 0xF0A0 != 0 {
            return Result::Err(MemoryResult::InvalidFlags);
        }
        let address = Pager::translate(virt);
        let pd = match self.get_pd_allocated(&address, flags, allocator) {
            Ok(pd) => pd,
            Err(e) => return Result::Err(e),
        };
        let mut pt = match pd.instantiate(address.pt) {
            Some(pt) => pt,
            None => return Result::Err(MemoryResult::AllocationError),
        };
        if pt.page_size_active() {
            pt.set_new_address(physical);
        } else {
            if pt.is_allocated() {
                match pt.deallocate(allocator) {
                    Some(e) => return Result::Err(e),
                    None => {}
                };
            }
            pt.flags |= PAGER_PAGE_SIZE;
            pt.set_new_address(physical);
        }
        return Result::Ok(Page::new(physical, virt));
    }

    pub fn is_active(&self, virt: u64) -> Result<bool, MemoryResult> {
        if virt & 0xFFF != 0 {
            return Result::Err(MemoryResult::InvalidAddress);
        }
        let address = Pager::translate(virt);
        let flags = unsafe { (*self.cr3.add(address.pdp as usize) & 0xFFF) as u16 };

        let pd = match self.get_pd_unallocated(&address, flags) {
            Ok(pd) => pd,
            Err(e) => return Result::Err(e),
        };
        let pt = match pd.instantiate(address.pt) {
            Some(pt) => pt,
            None => return Result::Err(MemoryResult::PagingError),
        };
        if pt.page_size_active() {
            return Result::Ok(pt.flags & 1 == 1);
        } else {
            let ptr = unsafe { (pt.address as *mut u64).add(address.page as usize) };
            return Result::Ok(unsafe { *ptr } & 1 == 1);
        }
    }

    pub fn is_kernel_only(&self, virt: u64) -> Result<bool, MemoryResult> {
        if virt & 0xFFF != 0 {
            return Result::Err(MemoryResult::InvalidAddress);
        }
        let address = Pager::translate(virt);
        let flags = unsafe { (*self.cr3.add(address.pdp as usize) & 0xFFF) as u16 };

        let pd = match self.get_pd_unallocated(&address, flags) {
            Ok(pd) => pd,
            Err(e) => return Result::Err(e),
        };
        let pt = match pd.instantiate(address.pt) {
            Some(pt) => pt,
            None => return Result::Err(MemoryResult::PagingError),
        };
        if pt.page_size_active() {
            return Result::Ok(pt.flags & PAGER_US == 0);
        } else {
            let ptr = unsafe { (pt.address as *mut u64).add(address.page as usize) };
            return Result::Ok(unsafe { *ptr } & PAGER_US as u64 == 0);
        }
    }

    pub fn deactivate(&self, virt: u64) -> Option<MemoryResult> {
        if virt & 0xFFF != 0 {
            return Option::Some(MemoryResult::InvalidAddress);
        }
        let address = Pager::translate(virt);
        let flags = unsafe { (*self.cr3.add(address.pdp as usize) & 0xFFF) as u16 };
        let pd = match self.get_pd_unallocated(&address, flags) {
            Ok(pd) => pd,
            Err(e) => return Option::Some(e),
        };
        let mut pt = match pd.instantiate(address.pt) {
            Some(pt) => pt,
            None => return Option::Some(MemoryResult::PagingError),
        };
        if pt.page_size_active() {
            pt.flags &= !1;
            unsafe {
                *pt.location = pt.address | pt.flags as u64;
            }
        } else {
            let ptr = unsafe { (pt.address as *mut u64).add(address.page as usize) };
            unsafe {
                *ptr &= !1;
            }
        }
        return Option::None;
    }
    pub fn activate(&self, virt: u64) -> Option<MemoryResult> {
        if virt & 0xFFF != 0 {
            return Option::Some(MemoryResult::InvalidAddress);
        }
        let address = Pager::translate(virt);
        let flags = unsafe { (*self.cr3.add(address.pdp as usize) & 0xFFF) as u16 };
        let pd = match self.get_pd_unallocated(&address, flags) {
            Ok(pd) => pd,
            Err(e) => return Option::Some(e),
        };
        let mut pt = match pd.instantiate(address.pt) {
            Some(pt) => pt,
            None => return Option::Some(MemoryResult::PagingError),
        };
        if pt.page_size_active() {
            pt.flags |= 1;
            unsafe {
                *pt.location = pt.address | pt.flags as u64;
            }
        } else {
            let ptr = unsafe { (pt.address as *mut u64).add(address.page as usize) };
            unsafe {
                *ptr |= 1;
            }
        }

        return Option::None;
    }

    pub fn switch_permission(&self, virt: u64) -> Option<MemoryResult> {
        if virt & 0xFFF != 0 {
            return Option::Some(MemoryResult::InvalidAddress);
        }
        let address = Pager::translate(virt);
        let flags = unsafe { (*self.cr3.add(address.pdp as usize) & 0xFFF) as u16 };
        let pd = match self.get_pd_unallocated(&address, flags) {
            Ok(pd) => pd,
            Err(e) => return Option::Some(e),
        };
        let mut pt = match pd.instantiate(address.pt) {
            Some(pt) => pt,
            None => return Option::Some(MemoryResult::PagingError),
        };
        if pt.page_size_active() {
            pt.flags ^= PAGER_US;
            unsafe {
                *pt.location = pt.address | pt.flags as u64;
            }
        } else {
            let ptr = unsafe { (pt.address as *mut u64).add(address.page as usize) };
            unsafe {
                *ptr ^= PAGER_US as u64;
            }
        }
        return Option::None;
    }

    pub fn get_physical(&self, virt: u64) -> Result<u64, MemoryResult> {
        if virt & 0xFFF != 0 {
            return Result::Err(MemoryResult::InvalidAddress);
        }
        let address = Pager::translate(virt);
        let flags = unsafe { (*self.cr3.add(address.pdp as usize) & 0xFFF) as u16 };
        let pd = match self.get_pd_unallocated(&address, flags) {
            Ok(pd) => pd,
            Err(e) => return Result::Err(e),
        };
        let pt = match pd.instantiate(address.pt) {
            Some(pt) => pt,
            None => return Result::Err(MemoryResult::PagingError),
        };
        if pt.page_size_active() {
            return Result::Ok(pt.address);
        } else {
            let ptr = unsafe { (pt.address as *mut u64).add(address.page as usize) };
            return Result::Ok(unsafe { *ptr & !0xFFF });
        }
    }

    //Returns, if the memory address is suited for a combination of operations (for example if it´s active and readwrite)
    pub fn is_suited_for(&self, virt: u64, write: bool, execution_mode: ExecutionMode) -> bool {
        if virt & 0xFFF != 0 {
            return false;
        }
        let address = Pager::translate(virt);
        let mut flags = 1;
        if write {
            flags |= PAGER_RW;
        }
        match execution_mode {
            ExecutionMode::Process => flags |= PAGER_US,
            ExecutionMode::Kernel => {}
        }
        let pdpt = PointerTable::new(PointerTableType::PDPT, self.cr3, address.pdp, 0);
        if pdpt.read_flags() & flags != flags {
            return false;
        }
        let pdt = match pdpt.instantiate(address.pd) {
            Some(pdt) => pdt,
            None => return false,
        };
        if pdt.read_flags() & flags != flags {
            return false;
        }
        if !pdt.page_size_active() {
            let pt = match pdt.instantiate(address.pt) {
                Some(pt) => pt,
                None => return false,
            };
            if pt.read_flags() & flags != flags {
                return false;
            }
            let page = unsafe { (pt.address as *const u64).add(address.page as usize) };
            if unsafe { *page as u16 } & flags != flags {
                return false;
            }
        }
        return true;
    }
}
