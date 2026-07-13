use core::{
    ffi::c_void,
    ptr::{self, null},
    slice,
};

use crate::{
    arch::{gdt::get_gdt_base, interrupts::get_idt_base},
    fixed_vaddrs::{LOADER_RESOURCES_FIXED_VADDR, PE32_PLUS_STACK_ADDRESS_FIXED_VADDR},
    hal::memory::{
        allocator::{Allocator, MemoryBlock, VirtualAllocator},
        pager::{PAGER_2MIB, PAGER_PCD, PAGER_PRESENT, PAGER_RW, PAGER_US, Pager},
    },
    processes::{
        launch_application,
        loader::{ExecutableLoadError, ExecutableType, FullExecutableImage, LoaderResources},
        pe::{constants::*, structures::*},
    },
    utils::{
        buffer::Buffer,
        c_style_length_check_u32, c_style_length_check_u64,
        memory::{memcmp_byte, memcpy, memset},
        traits::Region,
        unchecked_construct_utf8_string,
    },
    vfs::gfs::{GFS, file::GfsFile},
};
pub mod constants;
pub mod structures;

struct UnifiedOptionalHeader<'a> {
    major_linker_version: u8,
    minor_linker_version: u8,
    size_of_code: u32,
    size_of_initialized_data: u32,
    size_of_unitinialized_data: u32,
    address_of_entry_point: u32,
    base_of_code: u32,
    image_base: u64,
    section_alignment: u32,
    file_alignment: u32,
    major_operating_system_version: u16,
    minor_operating_system_version: u16,
    major_image_version: u16,
    minor_image_version: u16,
    major_subsystem_version: u16,
    minor_subsystem_version: u16,
    size_of_image: u32,
    size_of_headers: u32,
    checksum: u32,
    subsystem: u32,
    dll_characteristics: u16,
    size_of_stack_reserve: u64,
    size_of_stack_commit: u64,
    size_of_heap_reserve: u64,
    size_of_heap_commit: u64,
    rva_and_sizes: &'a [ImageDataDirectory],
}
struct PeCoff<'a> {
    pe_coff_characteristics: u16,
    optional_header: UnifiedOptionalHeader<'a>,
    section_table: &'a [ImageSectionHeader],
    is_64bit: bool,
}

struct PeEdata<'a> {
    export_address_table: &'a [u32],
    name_pointer_table: &'a [u32],
    ordinal_table: &'a [u16],
    name: &'a str,
    ordinal_base: u32,
}

struct PeIdata {
    length: u32,
    pointer: *const ImportDirectoryTable,
}

struct PeAddressTable {
    is_64bit: bool,
    memory: *const c_void,
}

struct PeImportLookupTableIterator {
    is_64bit: bool,
    length: u32,
    current: u32,
    memory: *const c_void,
}

struct PeItem {
    pub val: u64,
    pub is_64bit: bool,
}

pub struct PeRelocInformation {
    relocations: *const c_void,
    num_relocations: u32,
}

pub struct ImageSectionPaging {
    flags: u16,
    rva: u64,
    va: u64,
}
pub fn get_nt_dll_name<'a>(
    buffer: &'a mut (Buffer, bool),
    file: &GfsFile,
    physical_allocator: &mut Allocator,
    scheduler: u8,
) -> Result<&'a str, ExecutableLoadError> {
    *buffer = file.read(physical_allocator, unsafe { &*(&raw const GFS) }, scheduler);

    let pe = match PeCoff::load(&buffer.0, physical_allocator) {
        Ok(pe) => pe,
        Err(e) => return Result::Err(e),
    };

    if pe.pe_coff_characteristics & PE_IMAGE_FILE_DLL == 0 {
        return Result::Err(ExecutableLoadError::Malformed);
    }

    let edata = match pe.load_edata(&buffer.0) {
        Ok(edata) => edata,
        Err(e) => return Result::Err(e),
    };

    return Result::Ok(edata.name);
}

fn find_section_for_data_directory<'a>(
    data_directory: &ImageDataDirectory,
    sections: &'a [ImageSectionHeader],
) -> Option<&'a ImageSectionHeader> {
    for i in 0..sections.len() {
        let section = &sections[i as usize];
        if data_directory.virtual_address >= section.virtual_address
            && section.virtual_address + section.virtual_size
                >= data_directory.virtual_address + data_directory.size
        {
            return Option::Some(section);
        }
    }
    return Option::None;
}

fn find_section_for_rva<'a>(
    rva: u32,
    sections: &'a [ImageSectionHeader],
) -> Option<&'a ImageSectionHeader> {
    for i in 0..sections.len() {
        let section = &sections[i as usize];
        if rva >= section.virtual_address && section.virtual_address + section.virtual_size >= rva {
            return Option::Some(section);
        }
    }

    return Option::None;
}

impl ImportDirectoryTable {
    pub fn is_zero(&self) -> bool {
        return self.import_address_table_rva == 0
            && self.time_data_stamp == 0
            && self.forwarder_chain == 0
            && self.name_rva == 0
            && self.import_address_table_rva == 0;
    }
}

impl PeAddressTable {
    pub fn new(is_64bit: bool, memory: *const c_void) -> Self {
        return Self { is_64bit, memory };
    }

    pub fn write(&mut self, index: u32, val: u64) {
        if self.is_64bit {
            unsafe {
                *(self.memory.add(index as usize * 8) as *mut u64) = val;
            }
        } else {
            unsafe {
                *(self.memory.add(index as usize * 4) as *mut u32) = val as u32;
            }
        }
    }
}

impl PeImportLookupTableIterator {
    pub fn new(is_64bit: bool, memory: *const c_void) -> Self {
        let mut length = 0;
        if is_64bit {
            length = c_style_length_check_u64(memory as *const u64);
        } else {
            length = c_style_length_check_u32(memory as *const u32);
        }

        return Self {
            is_64bit,
            length,
            current: 0,
            memory,
        };
    }
}

impl Iterator for PeImportLookupTableIterator {
    type Item = PeItem;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current + 1 > self.length {
            return Option::None;
        } else {
            let val;

            if self.is_64bit {
                val = unsafe { *(self.memory as *const u64).add(self.current as usize) };
            } else {
                val = unsafe { *(self.memory as *const u32).add(self.current as usize) } as u64;
            }

            self.current += 1;
            return Option::Some(Self::Item {
                val,
                is_64bit: self.is_64bit,
            });
        }
    }
}

pub fn load_nt_dll<'a>(
    buffer: &'a mut (Buffer, bool),
    name: &str,
    kernel_allocator: &mut Allocator,
    physical_allocator: &mut Allocator,
    pager: &mut Pager,
    scheduler: u8,
    physical_memory_insertion: Option<(&mut [MemoryBlock], &mut u32)>,
    virtual_memory_insertion: Option<(&mut [MemoryBlock], &mut u32)>,
) -> Result<NtDll<'a>, ExecutableLoadError> {
    let resources = unsafe { &*(LOADER_RESOURCES_FIXED_VADDR as *const LoaderResources) };
    let file = match resources.find_dll(name) {
        Some(dll_file) => dll_file,
        None => return Result::Err(ExecutableLoadError::DependencyImportFailed),
    };

    match NtDll::load(
        file,
        physical_allocator,
        kernel_allocator,
        pager,
        scheduler,
        buffer,
        physical_memory_insertion,
        virtual_memory_insertion,
    ) {
        Ok(dll) => return Result::Ok(dll),
        Err(e) => {
            return Result::Err(e);
        }
    }
}
pub(in crate::processes) fn load_pe_executable(
    buffer: &Buffer,
    physical_allocator: &mut Allocator,
    kernel_allocator: &mut Allocator,
    virtual_allocator: &mut VirtualAllocator,
    scheduler: u8,
) -> Result<FullExecutableImage<'static>, ExecutableLoadError> {
    let pe = match PeCoff::load(buffer, physical_allocator) {
        Ok(pe) => pe,
        Err(e) => return Result::Err(e),
    };
    if pe.optional_header.size_of_stack_commit > PAGER_2MIB as u64 * 32
        || pe.optional_header.size_of_stack_reserve > PAGER_2MIB as u64 * 32
    {
        return Result::Err(ExecutableLoadError::Malformed);
    }

    let idata = match pe.load_idata(buffer) {
        Ok(idata) => idata,
        Err(e) => return Result::Err(e),
    };

    let memory_mb = match kernel_allocator.alloc_zero(2) {
        Ok(mb) => mb,
        Err(_e) => {
            return Result::Err(ExecutableLoadError::MemoryError);
        }
    };

    let physical_memory = unsafe {
        slice::from_raw_parts_mut(
            memory_mb.base as *mut MemoryBlock,
            0x1000 / size_of::<MemoryBlock>(),
        )
    };

    let virtual_memory = unsafe {
        slice::from_raw_parts_mut(
            (memory_mb.base + 0x1000) as *mut MemoryBlock,
            0x1000 / size_of::<MemoryBlock>(),
        )
    };

    let mut virtual_memory_index = 0u32;
    let mut physical_memory_index = 0u32;

    let cr3 = kernel_allocator.alloc_zero(1).unwrap();
    let mut pager = Pager::new(cr3.as_mut_ptr());

    let loader_resources = unsafe { &*(LOADER_RESOURCES_FIXED_VADDR as *const LoaderResources) };

    /* pages entry, so that launch_application can even transition between user application and kernel */
    match pager.page_4_kb(
        launch_application as *const c_void as u64,
        loader_resources.get_context_switch_physical(),
        PAGER_PRESENT,
        kernel_allocator,
    ) {
        Ok(_) => {}
        Err(_e) => {
            pager.release(kernel_allocator);
            return Result::Err(ExecutableLoadError::MemoryError);
        }
    }

    match pager.page_4_kb(
        unsafe { get_gdt_base() },
        loader_resources.get_gdt_physical(),
        PAGER_PRESENT,
        kernel_allocator,
    ) {
        Ok(_) => {}
        Err(_e) => {
            pager.release(kernel_allocator);
            return Result::Err(ExecutableLoadError::MemoryError);
        }
    }

    match pager.page_4_kb(
        unsafe { get_idt_base() },
        loader_resources.get_idt_physical(),
        PAGER_PRESENT,
        kernel_allocator,
    ) {
        Ok(_) => {}
        Err(_e) => {
            pager.release(kernel_allocator);
            return Result::Err(ExecutableLoadError::MemoryError);
        }
    }

    let mut val = pe.page_sections(
        buffer,
        &mut pager,
        kernel_allocator,
        Option::Some((physical_memory, &mut physical_memory_index)),
        Option::Some((virtual_memory, &mut virtual_memory_index)),
    );
    if let Option::Some(e) = val {
        pager.release(kernel_allocator);
        return Result::Err(e);
    }
    /* highest address occupied by application including dlls rounded up to 2 mb*/
    let mut mem_end = pe.optional_header.image_base
        + pe.section_table[pe.section_table.len() - 1].virtual_address as u64
        + pe.section_table[pe.section_table.len() - 1].virtual_size as u64;

    if mem_end % PAGER_2MIB as u64 != 0 {
        mem_end += PAGER_2MIB as u64 - mem_end % PAGER_2MIB as u64;
    }

    val = pe.iterate_idata(buffer, &idata, |name, lookup_iterator, address_table| {
        let mut dll_buffer = (Buffer::empty(), true);
        let scummy_dll_buffer_ptr = ptr::from_ref(&dll_buffer.0);
        let mut dll = match load_nt_dll(
            &mut dll_buffer,
            name,
            kernel_allocator,
            physical_allocator,
            &mut pager,
            scheduler,
            Option::Some((physical_memory, &mut physical_memory_index)),
            Option::Some((virtual_memory, &mut virtual_memory_index)),
        ) {
            Ok(dll) => dll,
            Err(e) => {
                if !dll_buffer.1 {
                    dll_buffer.0.release(physical_allocator);
                }
                return Option::Some(e);
            }
        };

        let mut dll_offset = 0;
        if pager.occupation_check(&dll.pager) {
            if dll.reloc_information.num_relocations == 0 {
                match dll.pager.release(kernel_allocator) {
                    Some(_e) => return Option::Some(ExecutableLoadError::MemoryError),
                    None => {}
                }
                if !dll_buffer.1 {
                    dll_buffer.0.release(physical_allocator);
                }

                return Option::Some(ExecutableLoadError::DependencyImportFailed);
            }
            let prev_base = dll.image_base;
            dll.reallocate(mem_end);
            dll_offset = dll.image_base - prev_base;
            mem_end += dll.section_table[dll.section_table.len() - 1].virtual_address as u64
                + dll.section_table[dll.section_table.len() - 1].virtual_size as u64;

            if mem_end % PAGER_2MIB as u64 != 0 {
                mem_end += PAGER_2MIB as u64 - mem_end % PAGER_2MIB as u64;
            }
        }
        match pager.bind_other(dll_offset, &dll.pager, kernel_allocator) {
            Some(_e) => {
                match dll.pager.release(kernel_allocator) {
                    Some(_e) => return Option::Some(ExecutableLoadError::MemoryError),
                    None => {}
                }

                if !dll_buffer.1 {
                    dll_buffer.0.release(physical_allocator);
                }

                return Option::Some(ExecutableLoadError::MemoryError);
            }
            None => {}
        }

        let dll_buffer_address = unsafe { (*scummy_dll_buffer_ptr).address() };

        while let Option::Some((index, entry)) = lookup_iterator.enumerate().next() {
            let import_by_ordinal;
            if entry.is_64bit {
                import_by_ordinal = entry.val & PE_PLUS_IMPORT_LOOKUP_USE_ORDINAL != 0;
            } else {
                import_by_ordinal = entry.val & PE_IMPORT_LOOKUP_USE_ORDINAL != 0;
            }

            let mut import_rva = 0;
            if !import_by_ordinal {
                let hint_name_table_rva = (entry.val & 0xFFFFFFFF) as u32;
                let section = match pe.find_section_for_rva(hint_name_table_rva) {
                    Some(section) => section,
                    None => return Option::Some(ExecutableLoadError::MissingSection),
                };
                let section_offset = hint_name_table_rva - section.virtual_address;
                let hint = unsafe {
                    *((buffer.address()
                        + section_offset as u64
                        + section.pointer_to_raw_data as u64) as *const u16)
                };

                let import_symbol_name_ptr = (buffer.address()
                    + 2
                    + section_offset as u64
                    + section.pointer_to_raw_data as u64)
                    as *const u8;

                let import_name = unchecked_construct_utf8_string(import_symbol_name_ptr);

                import_rva = match dll.get_export(dll_buffer_address, hint, import_name) {
                    Some(rva) => rva,
                    None => {
                        match pager.release(kernel_allocator) {
                            Some(_e) => return Option::Some(ExecutableLoadError::MemoryError),
                            None => {}
                        }

                        if !dll_buffer.1 {
                            dll_buffer.0.release(physical_allocator);
                        }

                        return Option::Some(ExecutableLoadError::ImportMissing);
                    }
                };
            } else {
                todo!("Import by ordinal\n");
            }
            address_table.write(index as u32, dll.image_base + import_rva);
        }

        if !dll_buffer.1 {
            dll_buffer.0.release(physical_allocator);
        }

        return Option::None;
    });

    if let Option::Some(e) = val {
        return Result::Err(e);
    } else {
        let stack_paddr_mb = match kernel_allocator
            .alloc((pe.optional_header.size_of_stack_commit / 0x1000) as u16)
        {
            Ok(mb) => mb,
            Err(_e) => {
                match pager.release(kernel_allocator) {
                    Some(_e) => return Result::Err(ExecutableLoadError::MemoryError),
                    None => {}
                }
                return Result::Err(ExecutableLoadError::MemoryError);
            }
        };
        match stack_paddr_mb.map(
            kernel_allocator,
            &mut pager,
            PE32_PLUS_STACK_ADDRESS_FIXED_VADDR - pe.optional_header.size_of_stack_commit,
            PAGER_RW | PAGER_PRESENT | PAGER_US,
        ) {
            Some(e) => {
                match pager.release(kernel_allocator) {
                    Some(_e) => return Result::Err(ExecutableLoadError::MemoryError),
                    None => {}
                }
                return Result::Err(ExecutableLoadError::MemoryError);
            }
            None => {}
        }

        virtual_memory[virtual_memory_index as usize] = MemoryBlock::new(
            pe.optional_header.size_of_stack_reserve,
            PE32_PLUS_STACK_ADDRESS_FIXED_VADDR,
        );
        virtual_memory_index += 1;
        physical_memory[physical_memory_index as usize] =
            MemoryBlock::new(pe.optional_header.size_of_stack_commit, stack_paddr_mb.base);
        physical_memory_index += 1;

        return Result::Ok(FullExecutableImage {
            cr3: pager.get_cr3() as u64,
            rbp_default: PE32_PLUS_STACK_ADDRESS_FIXED_VADDR,
            rsp_max: PE32_PLUS_STACK_ADDRESS_FIXED_VADDR - pe.optional_header.size_of_stack_reserve,
            entry_point: pe.optional_header.image_base
                + pe.optional_header.address_of_entry_point as u64,
            r#type: ExecutableType::Win,
            memory: memory_mb,
            physical_memory,
            virtual_memory,
            inserted_virtual_blocks: virtual_memory_index,
            inserted_physical_blocks: physical_memory_index,
        });
    }
}

impl<'b> PeCoff<'b> {
    fn find_section_for_rva(&self, rva: u32) -> Option<&ImageSectionHeader> {
        return find_section_for_rva(rva, self.section_table);
    }

    fn load_idata(&self, buffer: &Buffer) -> Result<PeIdata, ExecutableLoadError> {
        let header = &self.optional_header.rva_and_sizes[1];

        let import_directory_table_section = match self.find_section_for_rva(header.virtual_address)
        {
            Some(idata) => idata,
            None => return Result::Err(ExecutableLoadError::MissingSection),
        };
        let import_directory_table_offset =
            header.virtual_address - import_directory_table_section.virtual_address;

        let import_directory_table = unsafe {
            (buffer.address()
                + import_directory_table_section.pointer_to_raw_data as u64
                + import_directory_table_offset as u64) as *const ImportDirectoryTable
        };

        let mut length = 0;
        while (unsafe { !(*import_directory_table.add(length)).is_zero() }) {
            length += 1;
        }
        return Result::Ok(PeIdata {
            length: length as u32,
            pointer: import_directory_table,
        });
    }
    /**
     * section is aligned, and the page already has the appropiate flags
     * Also automatically fails, if the section has invalid flags
     */
    fn iterate_image_sections_for_paging(
        &self,
        mut func: impl FnMut(&ImageSectionHeader, &ImageSectionPaging) -> Option<ExecutableLoadError>,
    ) -> Option<ExecutableLoadError> {
        for i in 0..self.section_table.len() {
            let section = &self.section_table[i as usize];
            if section.characteristics
                & (IMAGE_SCN_TYPE_NO_PAD
                    | IMAGE_SCN_ALIGN_1BYTES
                    | IMAGE_SCN_ALIGN_2BYTES
                    | IMAGE_SCN_ALIGN_4BYTES
                    | IMAGE_SCN_ALIGN_8BYTES
                    | IMAGE_SCN_ALIGN_16BYTES
                    | IMAGE_SCN_ALIGN_32BYTES
                    | IMAGE_SCN_ALIGN_64BYTES
                    | IMAGE_SCN_ALIGN_128BYTES
                    | IMAGE_SCN_ALIGN_256BYTES
                    | IMAGE_SCN_ALIGN_512BYTES
                    | IMAGE_SCN_ALIGN_1024BYTES
                    | IMAGE_SCN_ALIGN_2048BYTES
                    | IMAGE_SCN_ALIGN_4096BYTES
                    | IMAGE_SCN_ALIGN_8192BYTES
                    | IMAGE_SCN_LNK_REMOVE
                    | IMAGE_SCN_LNK_COMDAT)
                != 0
            {
                return Option::Some(ExecutableLoadError::MalformedSection);
            }

            if section.virtual_address % self.optional_header.section_alignment != 0 {
                return Option::Some(ExecutableLoadError::InvalidAlignment);
            }

            if section.characteristics & (IMAGE_SCN_MEM_DISCARDABLE | IMAGE_SCN_MEM_NOT_PAGED) != 0
                || section.characteristics & IMAGE_SCN_MEM_READ == 0
            {
                /* Loader doesn´t put this page into memory.*/
                continue;
            }

            let mut flags = PAGER_US | PAGER_PRESENT;

            if section.characteristics & IMAGE_SCN_MEM_WRITE != 0 {
                flags |= PAGER_RW;
            }
            if section.characteristics & IMAGE_SCN_MEM_NOT_CACHED != 0 {
                flags |= PAGER_PCD;
            }

            let paging = ImageSectionPaging {
                rva: section.virtual_address as u64,
                va: self.optional_header.image_base + section.virtual_address as u64,
                flags,
            };

            if let Option::Some(e) = (func)(section, &paging) {
                return Option::Some(e);
            }
        }
        return Option::None;
    }

    /* &mut u32 => previous offset. Will be incremented
     */
    fn page_sections(
        &self,
        buffer: &Buffer,
        pager: &mut Pager,
        kernel_allocator: &mut Allocator,
        mut physical_memory_insertion: Option<(&mut [MemoryBlock], &mut u32)>,
        mut virtual_memory_insertion: Option<(&mut [MemoryBlock], &mut u32)>,
    ) -> Option<ExecutableLoadError> {
        let mut phys_base;
        if let Option::Some((_, ref base)) = physical_memory_insertion {
            phys_base = **base;
        } else {
            phys_base = 0;
        }
        let virt_base;
        if let Option::Some((_, ref base)) = virtual_memory_insertion {
            virt_base = **base;
        } else {
            virt_base = 0;
        }
        let val = self.iterate_image_sections_for_paging(|section, paging| {
            if section.characteristics
                & (IMAGE_SCN_CNT_UNINITIALIZED_DATA
                    | IMAGE_SCN_CNT_CODE
                    | IMAGE_SCN_CNT_INITIALIZED_DATA)
                != 0
            {
                let mut pages_4k = section.virtual_size / 0x1000;
                if section.virtual_size % 0x1000 != 0 {
                    pages_4k += 1;
                }

                let phys_mb = match kernel_allocator.alloc(pages_4k as u16) {
                    Ok(mb) => mb,
                    Err(_e) => {
                        return Option::Some(ExecutableLoadError::MemoryError);
                    }
                };

                if let Option::Some((phys_mbs, phys_curr)) = &mut physical_memory_insertion {
                    /* compresses block*/
                    let mut found = false;
                    for block in phys_mbs[phys_base as usize..**phys_curr as usize].iter_mut() {
                        if block.end() == phys_mb.base {
                            block.length += phys_mb.length;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        phys_mbs[**phys_curr as usize] = phys_mb;
                        **phys_curr += 1;
                    }
                }

                if let Option::Some((virt_mbs, virt_curr)) = &mut virtual_memory_insertion {
                    let mut found = false;
                    for block in virt_mbs[virt_base as usize..**virt_curr as usize].iter_mut() {
                        if block.end() == paging.va {
                            block.length += phys_mb.length;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        virt_mbs[**virt_curr as usize] =
                            MemoryBlock::new(phys_mb.length, paging.va);
                        **virt_curr += 1;
                    }
                }

                if section.virtual_size > PAGER_2MIB {
                    match phys_mb.map_efficient(kernel_allocator, pager, paging.va, paging.flags) {
                        Some(_) => {
                            let _ = kernel_allocator.free(&phys_mb);
                            return Option::Some(ExecutableLoadError::MemoryError);
                        }
                        None => {}
                    }
                } else {
                    match phys_mb.map(kernel_allocator, pager, paging.va, paging.flags) {
                        Some(_) => {
                            let _ = kernel_allocator.free(&phys_mb);
                            return Option::Some(ExecutableLoadError::MemoryError);
                        }
                        None => {}
                    }
                }

                if section.characteristics & (IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_CNT_CODE)
                    != 0
                {
                    unsafe {
                        let size_to_copy;
                        if section.virtual_size > section.size_of_raw_data {
                            size_to_copy = section.size_of_raw_data;
                        } else {
                            size_to_copy = section.virtual_size;
                        }
                        memcpy(
                            phys_mb.base as *mut c_void,
                            (buffer.address() + section.pointer_to_raw_data as u64)
                                as *const c_void,
                            size_to_copy,
                        );
                        if section.virtual_size > section.size_of_raw_data {
                            memset(
                                (phys_mb.base + size_to_copy as u64) as *mut c_void,
                                0,
                                section.virtual_size - section.size_of_raw_data,
                            );
                        }
                    };
                }
            }
            return Option::None;
        });

        return val;
    }

    fn iterate_sections(
        &self,
        mut func: impl FnMut(&ImageSectionHeader) -> Option<ExecutableLoadError>,
    ) -> Option<ExecutableLoadError> {
        for i in 0..self.section_table.len() {
            let opt = (func)(&self.section_table[i as usize]);
            if let Option::Some(ref e) = opt {
                return opt;
            }
        }
        return Option::None;
    }

    fn iterate_idata(
        &self,
        buffer: &Buffer,
        idata: &PeIdata,
        mut func: impl FnMut(
            &str,
            &mut PeImportLookupTableIterator,
            &mut PeAddressTable,
        ) -> Option<ExecutableLoadError>,
    ) -> Option<ExecutableLoadError> {
        for i in 0..idata.length as usize {
            let import_directory = unsafe { &*idata.pointer.add(i) };

            let name_section = match self.find_section_for_rva(import_directory.name_rva) {
                Some(section) => section,
                None => return Option::Some(ExecutableLoadError::MissingSection),
            };

            let import_address_table_section =
                match self.find_section_for_rva(import_directory.import_address_table_rva) {
                    Some(section) => section,
                    None => return Option::Some(ExecutableLoadError::MissingSection),
                };
            let import_address_table_offset = import_directory.import_address_table_rva
                - import_address_table_section.virtual_address;

            let import_lookup_table_section =
                match self.find_section_for_rva(import_directory.import_lookup_table_rva) {
                    Some(section) => section,
                    None => return Option::Some(ExecutableLoadError::MissingSection),
                };
            let import_lookup_table_offset = import_directory.import_lookup_table_rva
                - import_lookup_table_section.virtual_address;

            let name_offset = import_directory.name_rva - name_section.virtual_address;
            let name_ptr = (buffer.address()
                + name_offset as u64
                + name_section.pointer_to_raw_data as u64) as *const u8;

            let name = unchecked_construct_utf8_string(name_ptr);

            let mut lookup_iterator = PeImportLookupTableIterator::new(
                self.is_64bit,
                (buffer.address()
                    + import_lookup_table_offset as u64
                    + import_lookup_table_section.pointer_to_raw_data as u64)
                    as *const c_void,
            );

            let mut address_table = PeAddressTable::new(
                self.is_64bit,
                (buffer.address()
                    + import_address_table_offset as u64
                    + import_address_table_section.pointer_to_raw_data as u64)
                    as *const c_void,
            );
            if let Option::Some(e) = (func)(name, &mut lookup_iterator, &mut address_table) {
                return Option::Some(e);
            }
        }
        return Option::None;
    }

    fn load_edata<'a>(&self, buffer: &'a Buffer) -> Result<PeEdata<'a>, ExecutableLoadError> {
        let header = &self.optional_header.rva_and_sizes[0];

        let section = match self.find_section_for_rva(header.virtual_address) {
            Some(section) => section,
            None => return Result::Err(ExecutableLoadError::MissingSection),
        };

        if section.pointer_to_raw_data % self.optional_header.file_alignment != 0 {
            return Result::Err(ExecutableLoadError::MissingSection);
        }

        let edata_section = buffer.address()
            + section.pointer_to_raw_data as u64
            + (header.virtual_address - section.virtual_address) as u64;
        let export_directory_table = unsafe { &*(edata_section as *const ExportDirectoryTable) };
        if export_directory_table.export_flags != 0 {
            return Result::Err(ExecutableLoadError::Malformed);
        }

        let export_address_table_section = match self
            .find_section_for_rva(export_directory_table.export_address_table_pointer_rva)
        {
            Some(section) => section,
            None => return Result::Err(ExecutableLoadError::Malformed),
        };
        let export_address_table_offset = export_directory_table.export_address_table_pointer_rva
            - export_address_table_section.virtual_address;

        let ordinal_table_section =
            match self.find_section_for_rva(export_directory_table.ordinal_table_rva) {
                Some(section) => section,
                None => return Result::Err(ExecutableLoadError::Malformed),
            };
        let ordinal_table_offset =
            export_directory_table.ordinal_table_rva - ordinal_table_section.virtual_address;

        let name_pointer_table_section =
            match self.find_section_for_rva(export_directory_table.name_pointer_rva) {
                Some(section) => section,
                None => return Result::Err(ExecutableLoadError::MissingSection),
            };
        let name_pointer_table_offset =
            export_directory_table.name_pointer_rva - name_pointer_table_section.virtual_address;

        let name_section = match self.find_section_for_rva(export_directory_table.name_rva) {
            Some(section) => section,
            None => return Result::Err(ExecutableLoadError::Malformed),
        };
        let name_offset = export_directory_table.name_rva - name_section.virtual_address;

        let name_ptr: *const u8 = (buffer.address()
            + name_section.pointer_to_raw_data as u64
            + name_offset as u64) as *const u8;

        let export_address_table = unsafe {
            slice::from_raw_parts(
                (buffer.address()
                    + export_address_table_section.pointer_to_raw_data as u64
                    + export_address_table_offset as u64) as *const u32,
                export_directory_table.address_table_entries as usize,
            )
        };

        let ordinal_table = unsafe {
            slice::from_raw_parts(
                (buffer.address()
                    + ordinal_table_section.pointer_to_raw_data as u64
                    + ordinal_table_offset as u64) as *const u16,
                export_directory_table.number_of_name_pointers as usize,
            )
        };

        let name_pointer_table = unsafe {
            slice::from_raw_parts(
                (buffer.address()
                    + name_pointer_table_section.pointer_to_raw_data as u64
                    + name_pointer_table_offset as u64) as *const u32,
                export_directory_table.number_of_name_pointers as usize,
            )
        };
        return Result::Ok(PeEdata::<'a> {
            name: unchecked_construct_utf8_string(name_ptr),
            export_address_table,
            ordinal_table,
            name_pointer_table,
            ordinal_base: export_directory_table.ordinal_base,
        });
    }

    fn load_reloc(&self, buffer: &Buffer) -> Result<PeRelocInformation, ExecutableLoadError> {
        let reloc = &self.optional_header.rva_and_sizes[5];
        if reloc.size == 0 || reloc.virtual_address == 0 {
            return Result::Err(ExecutableLoadError::Unsupported);
        }

        let reloc_section = match self.find_section_for_rva(reloc.virtual_address) {
            Some(section) => section,
            None => return Result::Err(ExecutableLoadError::MissingSection),
        };
        let reloc_offset = reloc.virtual_address - reloc_section.virtual_address;

        let reloc_ptr = (buffer.address()
            + reloc_offset as u64
            + reloc_section.pointer_to_raw_data as u64) as *const c_void;

        let mut size_rem = reloc.size;
        let mut num_relocations = 0;
        while size_rem != 0 {
            let reloc_block = unsafe { &*(reloc_ptr as *const BaseRelocationBlock) };
            size_rem -= reloc_block.block_size;
            num_relocations += 1;
        }

        return Result::Ok(PeRelocInformation {
            relocations: reloc_ptr,
            num_relocations,
        });
    }

    fn load_optional_header<'a>(
        size: u64,
        buffer: &'a Buffer,
        offset: u64,
    ) -> Result<UnifiedOptionalHeader<'a>, ExecutableLoadError> {
        let optional_header =
            unsafe { &*((buffer.address() + offset) as *const Pe32OptionalHeader) };

        if optional_header.section_alignment != 0x1000
            && optional_header.section_alignment != optional_header.file_alignment
        {
            return Result::Err(ExecutableLoadError::Malformed);
        }

        if optional_header.win32_version_value != 0
            || optional_header.size_of_image % optional_header.section_alignment != 0
            || optional_header.size_of_headers % optional_header.file_alignment != 0
            || optional_header.loader_flags != 0
        {
            return Result::Err(ExecutableLoadError::Malformed);
        }
        let header_size = size_of::<Pe32OptionalHeader>() + 2;
        if header_size as u64
            + optional_header.number_of_rva_and_sizes as u64
                * size_of::<ImageDataDirectory>() as u64
            > size
        {
            return Result::Err(ExecutableLoadError::Malformed);
        }

        if optional_header.size_of_heap_commit % 0x1000 != 0
            || optional_header.size_of_heap_reserve != 0
            || optional_header.size_of_stack_commit % 0x1000 != 0
            || optional_header.size_of_stack_reserve % 0x1000 != 0
        {
            return Result::Err(ExecutableLoadError::Malformed);
        }

        return Result::Ok(UnifiedOptionalHeader {
            major_linker_version: optional_header.major_linker_version,
            minor_linker_version: optional_header.minor_linker_version,
            size_of_code: optional_header.size_of_code,
            size_of_initialized_data: optional_header.size_of_initialized_data,
            size_of_unitinialized_data: optional_header.size_of_uninitialized_data,
            address_of_entry_point: optional_header.address_of_entry_point,
            base_of_code: optional_header.base_of_code,
            image_base: optional_header.image_base as u64,
            section_alignment: optional_header.section_alignment,
            file_alignment: optional_header.file_alignment,
            major_operating_system_version: optional_header.major_operating_system_version,
            minor_operating_system_version: optional_header.minor_operating_system_version,
            major_image_version: optional_header.major_image_version,
            minor_image_version: optional_header.minor_image_version,
            major_subsystem_version: optional_header.major_subsystem_version,
            minor_subsystem_version: optional_header.minor_subsystem_version,
            size_of_image: optional_header.size_of_image,
            size_of_headers: optional_header.size_of_headers,
            checksum: optional_header.checksum,
            subsystem: optional_header.subsystem,
            dll_characteristics: optional_header.dll_characteristics,
            size_of_stack_reserve: optional_header.size_of_stack_reserve as u64,
            size_of_stack_commit: optional_header.size_of_stack_commit as u64,
            size_of_heap_reserve: optional_header.size_of_heap_reserve as u64,
            size_of_heap_commit: optional_header.size_of_heap_commit as u64,
            rva_and_sizes: unsafe {
                slice::from_raw_parts(
                    (buffer.address() + offset + size_of::<Pe32OptionalHeader>() as u64)
                        as *const ImageDataDirectory,
                    optional_header.number_of_rva_and_sizes as usize,
                )
            },
        });
    }

    fn load_plus_optional_header<'a>(
        size: u64,
        buffer: &'a Buffer,
        offset: u64,
    ) -> Result<UnifiedOptionalHeader<'a>, ExecutableLoadError> {
        let optional_header =
            unsafe { &*((buffer.address() + offset) as *const Pe32PlusOptionalHeader) };

        if optional_header.section_alignment != 0x1000
            && optional_header.section_alignment != optional_header.file_alignment
        {
            return Result::Err(ExecutableLoadError::Malformed);
        }
        if optional_header.win32_version_value != 0
            || optional_header.size_of_image % optional_header.section_alignment != 0
            || optional_header.size_of_headers % optional_header.file_alignment != 0
            || optional_header.loader_flags != 0
        {
            return Result::Err(ExecutableLoadError::Malformed);
        }
        let header_size = size_of::<Pe32PlusOptionalHeader>() + 2;
        if header_size as u64
            + optional_header.number_of_rva_and_sizes as u64
                * size_of::<ImageDataDirectory>() as u64
            > size
        {
            return Result::Err(ExecutableLoadError::Malformed);
        }

        if optional_header.size_of_heap_commit % 0x1000 != 0
            || optional_header.size_of_heap_reserve % 0x1000 != 0
            || optional_header.size_of_stack_commit % 0x1000 != 0
            || optional_header.size_of_stack_reserve % 0x1000 != 0
        {
            return Result::Err(ExecutableLoadError::Malformed);
        }

        return Result::Ok(UnifiedOptionalHeader {
            major_linker_version: optional_header.major_linker_version,
            minor_linker_version: optional_header.minor_linker_version,
            size_of_code: optional_header.size_of_code,
            size_of_initialized_data: optional_header.size_of_initialized_data,
            size_of_unitinialized_data: optional_header.size_of_uninitialized_data,
            address_of_entry_point: optional_header.address_of_entry_point,
            base_of_code: optional_header.base_of_code,
            image_base: optional_header.image_base,
            section_alignment: optional_header.section_alignment,
            file_alignment: optional_header.file_alignment,
            major_operating_system_version: optional_header.major_operating_system_version,
            minor_operating_system_version: optional_header.minor_operating_system_version,
            major_image_version: optional_header.major_image_version,
            minor_image_version: optional_header.minor_image_version,
            major_subsystem_version: optional_header.major_subsystem_version,
            minor_subsystem_version: optional_header.minor_subsystem_version,
            size_of_image: optional_header.size_of_image,
            size_of_headers: optional_header.size_of_headers,
            checksum: optional_header.checksum,
            subsystem: optional_header.subsystem as u32,
            dll_characteristics: optional_header.dll_characteristics,
            size_of_stack_reserve: optional_header.size_of_stack_reserve,
            size_of_stack_commit: optional_header.size_of_stack_commit,
            size_of_heap_reserve: optional_header.size_of_heap_commit,
            size_of_heap_commit: optional_header.size_of_heap_commit,
            rva_and_sizes: unsafe {
                slice::from_raw_parts(
                    (buffer.address() + offset + size_of::<Pe32PlusOptionalHeader>() as u64)
                        as *const ImageDataDirectory,
                    optional_header.number_of_rva_and_sizes as usize,
                )
            },
        });
    }
    /* Garantees, that section_alignment = 0x1000*/
    pub fn load<'a>(
        buffer: &'a Buffer,
        physical_allocator: &mut Allocator,
    ) -> Result<PeCoff<'a>, ExecutableLoadError> {
        let pe_offset = unsafe { *((buffer.address() + 0x3C) as *mut u32) };
        if pe_offset > buffer.get_size() as u32 {
            return Result::Err(ExecutableLoadError::Malformed);
        }
        let pe_header = unsafe { &*((buffer.address() + pe_offset as u64) as *const PeHeader) };
        if pe_header.size_of_optional_header == 0 {
            return Result::Err(ExecutableLoadError::Malformed);
        }
        if pe_header.machine != PE_IMAGE_FILE_MACHINE_AMD64 {
            return Result::Err(ExecutableLoadError::InvalidISA);
        }
        if pe_header.characteristics & PE_IMAGE_FILE_EXECUTABLE_IMAGE == 0
            || pe_header.characteristics & PE_IMAGE_FILE_32BIT_MACHINE != 0
            || pe_header.characteristics & PE_IMAGE_FILE_BYTES_REVERSED_HI != 0
        {
            return Result::Err(ExecutableLoadError::Malformed);
        }

        let indicator = unsafe {
            *((buffer.address() + pe_offset as u64 + size_of::<PeHeader>() as u64) as *mut u16)
        };

        let optional_block;

        let optional_offset = pe_offset as u64 + size_of::<PeHeader>() as u64 + 2;
        if indicator == 0x10b {
            optional_block = match PeCoff::load_optional_header(
                pe_header.size_of_optional_header as u64,
                buffer,
                optional_offset,
            ) {
                Ok(optional_block) => optional_block,
                Err(e) => return Result::Err(e),
            };
        } else if indicator == 0x20b {
            optional_block = match PeCoff::load_plus_optional_header(
                pe_header.size_of_optional_header as u64,
                buffer,
                optional_offset,
            ) {
                Ok(optional_block) => optional_block,
                Err(e) => return Result::Err(e),
            };
        } else {
            return Result::Err(ExecutableLoadError::Malformed);
        }

        let section_table = unsafe {
            slice::from_raw_parts(
                (buffer.address()
                    + pe_offset as u64
                    + size_of::<PeHeader>() as u64
                    + pe_header.size_of_optional_header as u64)
                    as *const ImageSectionHeader,
                pe_header.number_of_sections as usize,
            )
        };

        return Result::Ok(PeCoff {
            pe_coff_characteristics: pe_header.characteristics,
            optional_header: optional_block,
            section_table,
            is_64bit: indicator == 0x20b,
        });
    }
}

pub struct NtDll<'a> {
    export_data: PeEdata<'a>,
    section_table: &'a [ImageSectionHeader],
    reloc_information: PeRelocInformation,

    image_base: u64,
    pager: Pager,
}

impl NtDll<'_> {
    pub fn load<'a>(
        file: &GfsFile,
        physical_allocator: &mut Allocator,
        kernel_allocator: &mut Allocator,
        pager: &mut Pager,
        scheduler: u8,
        buffer: &'a mut (Buffer, bool),
        physical_memory_insertion: Option<(&mut [MemoryBlock], &mut u32)>,
        virtual_memory_insertion: Option<(&mut [MemoryBlock], &mut u32)>,
    ) -> Result<NtDll<'a>, ExecutableLoadError> {
        *buffer = file.read(physical_allocator, unsafe { &*(&raw const GFS) }, scheduler);

        let pe = match PeCoff::load(&buffer.0, physical_allocator) {
            Ok(pe) => pe,
            Err(e) => {
                return Result::Err(e);
            }
        };
        if pe.pe_coff_characteristics & PE_IMAGE_FILE_DLL == 0 {
            return Result::Err(ExecutableLoadError::Malformed);
        }

        let edata = match pe.load_edata(&buffer.0) {
            Ok(edata) => edata,
            Err(e) => return Result::Err(e),
        };

        let reloc_information;
        match pe.load_reloc(&buffer.0) {
            Ok(reloc_info) => reloc_information = reloc_info,
            Err(e) => {
                if let ExecutableLoadError::Unsupported = e {
                    reloc_information = PeRelocInformation {
                        num_relocations: 0,
                        relocations: null(),
                    }
                } else {
                    return Result::Err(e);
                }
            }
        }

        let cr3_mb = match kernel_allocator.alloc_zero(1) {
            Ok(cr3) => cr3,
            Err(e) => return Result::Err(ExecutableLoadError::MemoryError),
        };

        let mut pager = Pager::new(cr3_mb.as_mut_ptr());

        match pe.page_sections(
            &buffer.0,
            &mut pager,
            kernel_allocator,
            physical_memory_insertion,
            virtual_memory_insertion,
        ) {
            Some(e) => {
                match pager.release(kernel_allocator) {
                    Some(_e) => return Result::Err(ExecutableLoadError::MemoryError),
                    None => {}
                }
                return Result::Err(e);
            }
            None => {}
        }
        return Result::Ok(NtDll::<'a> {
            reloc_information,
            export_data: edata,
            section_table: pe.section_table,
            image_base: pe.optional_header.image_base,
            pager: pager,
        });
    }

    pub fn get_export(&self, buffer_address: u64, hint: u16, import_name: &str) -> Option<u64> {
        if self.export_data.name_pointer_table.len() as u16 > hint {
            let name_rva = self.export_data.name_pointer_table[hint as usize];
            let name_section = match find_section_for_rva(name_rva, self.section_table) {
                Some(section) => section,
                None => {
                    return Option::None;
                }
            };
            let name_offset = name_rva - name_section.virtual_address;
            let name_ptr = (buffer_address
                + name_offset as u64
                + name_section.pointer_to_raw_data as u64) as *const u8;

            let name = unchecked_construct_utf8_string(name_ptr);
            if name.len() == import_name.len() {
                unsafe {
                    if memcmp_byte(name.as_ptr(), import_name.as_ptr(), name.len() as u32) {
                        return Option::Some(
                            self.export_data.export_address_table[hint as usize] as u64,
                        );
                    }
                }
            }
        }
        for i in 0..self.export_data.name_pointer_table.len() {
            let name_rva = self.export_data.name_pointer_table[i as usize];
            let name_section = match find_section_for_rva(name_rva, self.section_table) {
                Some(section) => section,
                None => {
                    return Option::None;
                }
            };
            let name_offset = name_rva - name_section.virtual_address;
            let name_ptr = (buffer_address
                + name_offset as u64
                + name_section.pointer_to_raw_data as u64) as *const u8;

            let name = unchecked_construct_utf8_string(name_ptr);
            if name.len() != import_name.len() {
                continue;
            }
            unsafe {
                if memcmp_byte(name.as_ptr(), import_name.as_ptr(), name.len() as u32) {
                    return Option::Some(self.export_data.export_address_table[i as usize] as u64);
                }
            }
        }
        return Option::None;
    }

    pub fn reallocate(&mut self, new_base: u64) -> Option<ExecutableLoadError> {
        /* TODO: this!*/
        self.image_base = new_base; /* last in function*/
        todo!()
    }
}
