use core::{
    ffi::{c_uchar, c_void},
    ptr::{addr_of, slice_from_raw_parts},
    slice,
};

use crate::{
    aml::definitions::{DataRefObject, TermArgInt::Add},
    arch::{gdt::get_gdt_base, interrupts::get_idt_base},
    drivers::usb::independent::UsbControllerType::UHC,
    fixed_vaddrs::LOADER_NT64_RESOURCES_MAPPINGS_ARRAY_FIXED_VADDR,
    hal::{
        memory::{
            allocator::{Allocator, MemoryBlock, VirtualAllocator},
            pager::{Page, Pager},
        },
        print::simple_kernel_panic,
    },
    processes::{
        launch_application,
        pe::{get_nt_dll_name, load_pe_executable},
    },
    utils::{buffer::Buffer, invalid_mut_slice, invalid_slice, traits::Region},
    vfs::gfs::{GFS, GfsType, file::GfsFile},
};

pub struct NT64LoaderResources {
    pub memory: [MemoryBlock; 2],

    mappings: &'static mut [(&'static str, &'static GfsFile)],
    string_used: u32,
    used: u16,
}

impl NT64LoaderResources {
    pub const MAPPING_MEMORY_INDEX: usize = 0;
    pub const STRING_MEMORY_INDEX: usize = 1;
}
/* from the boot partition */
pub static NT64_LIBRARIES_PATH: &'static str = "/sys/Windows/";

pub struct LoaderResources {
    nt64_dlls: NT64LoaderResources,
    context_switch_map_physical: u64,
    gdt_physical: u64,
    idt_physical: u64,
}

impl LoaderResources {
    pub fn get_context_switch_physical(&self) -> u64 {
        return self.context_switch_map_physical;
    }

    pub fn get_gdt_physical(&self) -> u64 {
        return self.gdt_physical;
    }
    pub fn get_idt_physical(&self) -> u64 {
        return self.idt_physical;
    }

    pub fn new(physical_allocator: &mut Allocator, pager: &Pager) -> LoaderResources {
        let mappings_memory = physical_allocator.alloc_zero(2).unwrap();
        let string_memory = physical_allocator.alloc_zero(2).unwrap();

        let launch_application_addr = launch_application as *const c_void as u64;

        /* asserts that launch_application is located at the start of the page */
        assert!(launch_application_addr as u64 % 0x1000 == 0);

        let phys = match pager.get_physical(launch_application_addr as u64) {
            Ok(phys) => phys,
            Err(e) => simple_kernel_panic(
                "LoaderResources/new",
                "Could not find physical address of 'launch_application'\n",
            ),
        };

        let gdt_vaddr = unsafe { get_gdt_base() };
        let idt_vaddr = unsafe { get_idt_base() };

        let gdt_physical = match pager.get_physical(gdt_vaddr) {
            Ok(phys) => phys,
            Err(_e) => simple_kernel_panic(
                "LoaderResources/new",
                "Could not get physical address of the gdt\n",
            ),
        };

        let idt_physical = match pager.get_physical(idt_vaddr) {
            Ok(phys) => phys,
            Err(_e) => simple_kernel_panic(
                "LoaderResources/new",
                "Could not get physical address of the idt\n",
            ),
        };

        return LoaderResources {
            nt64_dlls: NT64LoaderResources {
                memory: [mappings_memory, string_memory],
                mappings: unsafe {
                    slice::from_raw_parts_mut(
                        LOADER_NT64_RESOURCES_MAPPINGS_ARRAY_FIXED_VADDR
                            as *mut (&'static str, &'static GfsFile),
                        mappings_memory.length as usize
                            / size_of::<(&'static str, &'static GfsFile)>(),
                    )
                },
                used: 0,
                string_used: 0,
            },
            context_switch_map_physical: phys,
            idt_physical,
            gdt_physical,
        };
    }

    pub fn find_dll(&self, name: &str) -> Option<&GfsFile> {
        for i in 0..self.nt64_dlls.used {
            let mapping = &self.nt64_dlls.mappings[i as usize];
            if mapping.0 == name {
                return Option::Some(mapping.1);
            }
        }
        return Option::None;
    }

    #[allow(static_mut_refs)]
    pub fn load_nt64_system_libraries(
        &mut self,
        virtual_allocator: &mut VirtualAllocator,
        physical_allocator: &mut Allocator,
    ) {
        let directory = unsafe {
            GFS.discover_directory(NT64_LIBRARIES_PATH, physical_allocator, 1);
            match GFS.get_directory(NT64_LIBRARIES_PATH) {
                Ok(directory) => directory,
                Err(_e) => simple_kernel_panic(
                    "LoaderResources/load_nt64_system_libraries",
                    "could not get directory containing libraries\n",
                ),
            }
        };

        directory
            .get_entries()
            .for_each(unsafe { &*GFS.get_manager() }, |name, link| {
                if name.ends_with("dll")
                    && let GfsType::File = link.link_type()
                {
                    let file = unsafe { &mut *GFS.ref_file_mut(link.index()) };
                    file.virtualize_virtual(
                        unsafe { &GFS },
                        physical_allocator,
                        virtual_allocator,
                        1,
                    );
                    file.set_system_access();
                    file.remove_write_permission();

                    let mut buffer = (Buffer::empty(), false);

                    let name = match get_nt_dll_name(&mut buffer, file, physical_allocator, 1) {
                        Ok(name) => name,
                        Err(_e) => simple_kernel_panic(
                            "LoaderResources/load_nt64_system_libraries",
                            "Could not get name of dll\n",
                        ),
                    };

                    if self.nt64_dlls.used as usize + 1 > self.nt64_dlls.mappings.len() {
                        todo!("Reallocate!\n");
                    }

                    if self.nt64_dlls.string_used + name.len() as u32
                        > self.nt64_dlls.memory[NT64LoaderResources::STRING_MEMORY_INDEX]
                            .get_length() as u32
                    {
                        todo!("Reallocate!\n");
                    }

                    let str = unsafe {
                        str::from_utf8_unchecked_mut(slice::from_raw_parts_mut(
                            (self.nt64_dlls.memory[NT64LoaderResources::STRING_MEMORY_INDEX]
                                .get_base()
                                + self.nt64_dlls.string_used as u64)
                                as *mut u8,
                            name.len(),
                        ))
                    };
                    unsafe { str.as_bytes_mut().copy_from_slice(name.as_bytes()) };

                    let mapping = &mut self.nt64_dlls.mappings[self.nt64_dlls.used as usize];
                    mapping.0 = str;
                    mapping.1 = file;
                    self.nt64_dlls.used += 1;
                    self.nt64_dlls.string_used += name.len() as u32;

                    if !buffer.1 {
                        buffer.0.release(physical_allocator);
                    }
                }
            });

        /* For each DLL => analyze it and package it into RawNtDll*/
    }

    pub fn ref_nt64(&self) -> &NT64LoaderResources {
        return &self.nt64_dlls;
    }
}

pub enum ExecutableLoadError {
    Unsupported,
    Malformed,
    InvalidISA,
    NoIdata,
    MissingSection,
    MalformedSection,
    InvalidAlignment,
    DependencyImportFailed,
    ImportMissing,
    MemoryError,
}

pub enum ExecutableType {
    Lin,
    Win,
    Empty,
}

/*
 * Info: Memory of prozess cannot be identity mapped
 * Info: change Kernel to upper half.
 */
pub struct FullExecutableImage<'a> {
    pub cr3: u64,
    pub rbp_default: u64, // base of stack
    pub rsp_max: u64,     // maximum of stack
    pub entry_point: u64,
    pub r#type: ExecutableType,

    pub memory: MemoryBlock, // storage location of the memory blocks of physical_memory
    pub physical_memory: &'a mut [MemoryBlock],
    pub virtual_memory: &'a mut [MemoryBlock], // virtual memory equivalent of physical_memory from the kernel side

    pub inserted_physical_blocks: u32,
    pub inserted_virtual_blocks: u32,
}

impl<'a> FullExecutableImage<'a> {
    pub fn extract(self) -> ExecutableImage<'a> {
        return ExecutableImage {
            r#type: self.r#type,
            memory: self.memory,
            physical_memory: self.physical_memory,
            virtual_memory: self.virtual_memory,
            physical_blocks_present: self.inserted_physical_blocks,
            virtual_blocks_present: self.inserted_virtual_blocks,
        };
    }
}

pub struct ExecutableImage<'a> {
    r#type: ExecutableType,
    memory: MemoryBlock,
    physical_memory: &'a [MemoryBlock],
    virtual_memory: &'a [MemoryBlock],

    physical_blocks_present: u32,
    virtual_blocks_present: u32,
}

impl<'a> ExecutableImage<'a> {
    pub const fn empty() -> ExecutableImage<'a> {
        return ExecutableImage {
            memory: MemoryBlock::empty(),
            physical_memory: invalid_slice(),
            virtual_memory: invalid_slice(),
            r#type: ExecutableType::Empty,
            virtual_blocks_present: 0,
            physical_blocks_present: 0,
        };
    }
}
/* should both support elf and exe*/
pub fn load_executable_file(
    buffer: &Buffer,
    physical_allocator: &mut Allocator,
    kernel_allocator: &mut Allocator,
    virtual_allocator: &mut VirtualAllocator,
    scheduler: u8,
) -> Result<FullExecutableImage<'static>, ExecutableLoadError> {
    let buffer_content = buffer.as_slice();

    if buffer_content[0] == 'M' as u8 && buffer_content[1] == 'Z' as u8 {
        return load_pe_executable(
            buffer,
            physical_allocator,
            kernel_allocator,
            virtual_allocator,
            scheduler,
        );
    }

    return Result::Err(ExecutableLoadError::Unsupported);
}
