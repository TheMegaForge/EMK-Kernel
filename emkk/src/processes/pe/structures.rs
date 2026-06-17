use core::ffi::c_uchar;

#[repr(C, packed)]
pub struct PeHeader {
    pub magic: u32,
    pub machine: u16,
    pub number_of_sections: u16,
    pub time_date_stamp: u32,
    pub pointer_to_symbol_table: u32,
    pub number_of_symbols: u32,
    pub size_of_optional_header: u16,
    pub characteristics: u16,
}

#[repr(C, packed)]
pub struct Pe32OptionalHeader {
    pub major_linker_version: u8,
    pub minor_linker_version: u8,
    pub size_of_code: u32,
    pub size_of_initialized_data: u32,
    pub size_of_uninitialized_data: u32,
    pub address_of_entry_point: u32,
    pub base_of_code: u32,
    pub base_of_data: u32,
    pub image_base: u32,
    pub section_alignment: u32,
    pub file_alignment: u32,
    pub major_operating_system_version: u16,
    pub minor_operating_system_version: u16,
    pub major_image_version: u16,
    pub minor_image_version: u16,
    pub major_subsystem_version: u16,
    pub minor_subsystem_version: u16,
    pub win32_version_value: u16,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub checksum: u32,
    pub subsystem: u32,
    pub dll_characteristics: u16,
    pub size_of_stack_reserve: u32,
    pub size_of_stack_commit: u32,
    pub size_of_heap_reserve: u32,
    pub size_of_heap_commit: u32,
    pub loader_flags: u32,
    pub number_of_rva_and_sizes: u32,
}
#[repr(C, packed)]
pub struct Pe32PlusOptionalHeader {
    pub major_linker_version: u8,
    pub minor_linker_version: u8,
    pub size_of_code: u32,
    pub size_of_initialized_data: u32,
    pub size_of_uninitialized_data: u32,
    pub address_of_entry_point: u32,
    pub base_of_code: u32,
    pub image_base: u64,
    pub section_alignment: u32,
    pub file_alignment: u32,
    pub major_operating_system_version: u16,
    pub minor_operating_system_version: u16,
    pub major_image_version: u16,
    pub minor_image_version: u16,
    pub major_subsystem_version: u16,
    pub minor_subsystem_version: u16,
    pub win32_version_value: u32,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub checksum: u32,
    pub subsystem: u16,
    pub dll_characteristics: u16,
    pub size_of_stack_reserve: u64,
    pub size_of_stack_commit: u64,
    pub size_of_heap_reserve: u64,
    pub size_of_heap_commit: u64,
    pub loader_flags: u32,
    pub number_of_rva_and_sizes: u32,
}

#[repr(C, packed)]
pub struct ImageDataDirectory {
    pub virtual_address: u32, // rva
    pub size: u32,
}
#[repr(C, packed)]
pub struct ImageSectionHeader {
    pub name: [c_uchar; 8],
    pub virtual_size: u32,
    pub virtual_address: u32, // rva
    pub size_of_raw_data: u32,
    pub pointer_to_raw_data: u32,
    pub pointer_to_relocations: u32,
    pub pointer_to_line_numbers: u32,
    pub number_of_relocations: u16,
    pub number_of_line_numbers: u16,
    pub characteristics: u32,
}

#[repr(C, packed)]
pub struct COFFSymbolTableEntry {
    pub name: [c_uchar; 8],
    pub value: u32,
    pub section_number: u16,
    pub r#type: u16,
    pub storage_class: u8,
    pub number_of_aux_symbols: u8,
}

#[repr(C, packed)]
pub struct COFFImportDirectoryTableEntry {
    pub import_lookup_table_rva: u32,
    pub time_date_stamp: u32,
    pub forward_chainer: u32,
    pub name_rva: u32,
    pub import_address_table_rva: u32,
}

impl COFFImportDirectoryTableEntry {
    pub fn is_zero(&self) -> bool {
        return self.import_address_table_rva == 0
            && self.time_date_stamp == 0
            && self.forward_chainer == 0
            && self.name_rva == 0
            && self.import_address_table_rva == 0;
    }
}

#[repr(C)]
pub struct ExportDirectoryTable {
    pub export_flags: u32,
    pub time_data_stemp: u32,
    pub major_version: u16,
    pub minor_version: u16,
    pub name_rva: u32,
    pub ordinal_base: u32,
    pub address_table_entries: u32,
    pub number_of_name_pointers: u32,
    pub export_address_table_pointer_rva: u32,
    pub name_pointer_rva: u32,
    pub ordinal_table_rva: u32,
}
#[repr(C)]
pub struct ImportDirectoryTable {
    pub import_lookup_table_rva: u32,
    pub time_data_stamp: u32,
    pub forwarder_chain: u32,
    pub name_rva: u32,
    pub import_address_table_rva: u32,
}

#[repr(C)]
pub struct BaseRelocationBlock {
    pub page_rva: u32,
    pub block_size: u32,
}
