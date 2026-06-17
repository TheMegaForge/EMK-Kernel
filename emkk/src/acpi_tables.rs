use crate::{
    hal::{
        SystemTable,
        memory::pager::{PAGER_PRESENT, page_align},
        print::{Module, simple_kernel_panic},
    },
    info, success,
    utils::allocators::PageAllocator,
};
use core::{
    ffi::{c_char, c_void},
    mem::offset_of,
    ptr::{addr_of, null},
};

#[derive(PartialEq, Clone)]
pub enum AcpiTableId {
    APIC,
    FACP,
    SRAT,
    SSDT,
    DSDT,
    HPET,
    MCFG,
    UNUSED,
}

#[allow(dead_code)]
pub struct AcpiTable {
    table: AcpiTableId,
    ptr: *const c_void,
    length: u32,
}

impl AcpiTable {
    pub fn new(table: AcpiTableId, ptr: *const c_void, length: u32) -> Self {
        return Self { table, ptr, length };
    }

    pub fn get_ptr(&self) -> *const c_void {
        return self.ptr;
    }
}

#[repr(C, packed)]
pub struct DSDT {
    pub header: descriptor_table,
    pub definition_table: [u8; 0],
}

pub struct AcpiTables {
    number_of_tables: u8,
    tables: PageAllocator<AcpiTable>,
}
#[repr(C, packed)]
struct Rsdp {
    signature: [c_char; 8],
    checksum: u8,
    oemid: [c_char; 6],
    revision: u8,
    rsdt_address: u32,
}
#[repr(C, packed)]
struct Rsdt {
    signature: [c_char; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [c_char; 6],
    oemtableid: [c_char; 8],
    oemrevision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[repr(C, packed)]
pub struct descriptor_table {
    signature: [c_char; 4],  // 4
    length: u32,             // 4 => 8
    revision: u8,            // 1
    checksum: u8,            // 1
    oemid: [c_char; 6],      // 6 => 8
    oemtableid: [c_char; 8], // 8 => 8
    oemrevision: u32,        // 4
    creatorid: u32,          // 4 => 8
    creator_revision: u32,   // 4
}

impl descriptor_table {
    pub fn get_length(&self) -> u32 {
        return self.length;
    }
    pub fn end_of_table(&self) -> *const c_void {
        return unsafe {
            (addr_of!(self) as *const c_void)
                .add(offset_of!(descriptor_table, creator_revision) + 4)
        };
    }
}

#[repr(C, packed)]
pub struct GenericAddressStructure {
    address_space: u8,
    bit_width: u8,
    bit_offset: u8,
    access_size: u8,
    pub address: u64,
}

#[repr(C)]
pub struct FADT {
    header: descriptor_table,
    firmware_ctrl: u32,
    pub dsdt: u32,

    // field used in ACPI 1.0; no longer in use, for compatibility only
    reserved: u8,

    preferred_power_management_profile: u8,
    sci_interrupt: u16,
    smi_command_port: u32,
    acpi_enable: u8,
    acpi_disable: u8,
    s4_bios_req: u8,
    pstate_control: u8,
    pm1a_event_block: u32,
    pm1b_event_block: u32,
    pm1a_control_block: u32,
    pm1b_control_block: u32,
    pm2_control_block: u32,
    pmtimer_block: u32,
    gpe_0_block: u32,
    gpe_1_block: u32,
    pm_1_event_length: u8,
    pm_1_control_length: u8,
    pm_2_control_length: u8,
    pmtimer_length: u8,
    gpe_0_length: u8,
    gpe_1_length: u8,
    gpe_1_base: u8,
    c_state_control: u8,
    worst_c2_latency: u16,
    worst_c3_latency: u16,
    flush_size: u16,
    flush_stride: u16,
    duty_offset: u8,
    duty_width: u8,
    day_alarm: u8,
    month_alarm: u8,
    century: u8,

    // reserved in ACPI 1.0; used since ACPI 2.0+
    boot_architecture_flags: u16,

    reserved2: u8,
    flags: u32,

    // 12 byte structure; see below for details
    reset_reg: GenericAddressStructure,

    reset_value: u8,
    reserved3: [u8; 3],

    // 64bit pointers - Available on ACPI 2.0+
    x_firmware_control: u64,
    x_dsdt: u64,

    x_pm1a_event_block: GenericAddressStructure,
    x_pm1b_event_block: GenericAddressStructure,
    x_pm1a_control_block: GenericAddressStructure,
    x_pm1b_control_block: GenericAddressStructure,
    x_pm2_control_block: GenericAddressStructure,
    x_pmtimer_block: GenericAddressStructure,
    x_gpe_0_block: GenericAddressStructure,
    x_gpe_1_block: GenericAddressStructure,
}

pub const APIC_SIGNATURE_U32: u32 = 0x43495041;
pub const FACP_SIGNATURE_U32: u32 = 0x50434146;
pub const SRAT_SIGNATURE_U32: u32 = 0x54415253;
pub const SSDT_SIGNATURE_U32: u32 = 0x54445353;
pub const HPET_SIGNATURE_U32: u32 = 0x54455048;
pub const MCFG_SIGNATURE_U32: u32 = 0x4746434D;

//TODO: this! + PCI!
impl AcpiTables {
    pub fn initialize(&mut self, system_table: &mut SystemTable, rsdp: *const c_void) {
        self.tables
            .free(&mut system_table.virtual_allocator.allocator);
        let mut module: Module<'static> = Module::new("Acpi Tables");
        info!(module, "began initialization\n");
        let _rsdp = rsdp as *const Rsdp;
        match system_table.page_4k(page_align(_rsdp), page_align(_rsdp), PAGER_PRESENT) {
            Ok(_) => {}
            Err(_e) => {
                simple_kernel_panic(module.name(), "Could not page RSDP\n");
            }
        };
        let rsdt = unsafe { (*_rsdp).rsdt_address } as *const Rsdt;
        match system_table.page_4k(page_align(rsdt), page_align(rsdt), PAGER_PRESENT) {
            Ok(_) => {}
            Err(_e) => {
                simple_kernel_panic(module.name(), "Could not page RSDT\n");
            }
        };
        let num_tables = (unsafe { (*rsdt).length } - size_of::<Rsdt>() as u32) / 4;
        self.tables
            .initialize(&mut system_table.virtual_allocator.allocator, num_tables);
        let addr = unsafe { &raw const (*rsdt).creator_revision as u64 } + 4;
        for i in 0..num_tables {
            let dt =
                { unsafe { *((addr + (i * 4) as u64) as *const u32) } } as *const descriptor_table;
            match system_table.page_4k(page_align(dt) as u64, page_align(dt) as u64, PAGER_PRESENT)
            {
                Ok(_) => {}
                Err(_e) => {
                    simple_kernel_panic(module.name(), "Could not page descriptor_table\n");
                }
            }

            let mut pages = 0;
            if unsafe { (*dt).length } % 0x1000 != 0 {
                pages = 1;
            }
            pages += unsafe { (*dt).length } / 0x1000;
            for i in 1..pages {
                match system_table.page_4k(
                    page_align(dt) + (i * 0x1000) as u64,
                    page_align(dt) as u64 + (i * 0x1000) as u64,
                    PAGER_PRESENT,
                ) {
                    Ok(_) => {}
                    Err(_e) => {
                        simple_kernel_panic(module.name(), "Could not page descriptor_table\n");
                    }
                }
            }

            let sig: u32 = unsafe {
                *((dt as *const c_void).add(offset_of!(descriptor_table, signature)) as *const u32)
            };
            let id;
            if sig == APIC_SIGNATURE_U32 {
                id = AcpiTableId::APIC;
                info!(module, "Found 'APIC' Table\n");
            } else if sig == FACP_SIGNATURE_U32 {
                id = AcpiTableId::FACP;
                info!(module, "Found 'FACP' Table\n");
                let fadt = dt as *const FADT;
                let dsdt = unsafe { (*fadt).dsdt } as u64 as *const descriptor_table;

                match system_table.page_4k(page_align(dsdt), page_align(dsdt), PAGER_PRESENT) {
                    Ok(_) => {}
                    Err(_e) => {
                        simple_kernel_panic(module.name(), "Could not page DSDT\n");
                    }
                };

                info!(module, "Found 'DSDT' in 'FADT'\n");
                self.tables.push_back(AcpiTable::new(
                    AcpiTableId::DSDT,
                    dsdt as *const c_void,
                    unsafe { (*dsdt).length },
                ));
            } else if sig == SRAT_SIGNATURE_U32 {
                id = AcpiTableId::SRAT;
                info!(module, "Found 'SRTA' Table\n");
            } else if sig == SSDT_SIGNATURE_U32 {
                id = AcpiTableId::SSDT;
                info!(module, "Found 'SSDT' Table\n");
            } else if sig == HPET_SIGNATURE_U32 {
                id = AcpiTableId::HPET;
                info!(module, "Found 'HPET' Table\n");
            } else if sig == MCFG_SIGNATURE_U32 {
                id = AcpiTableId::MCFG;
                info!(module, "Found 'MCFG' Table\n");
            } else {
                id = AcpiTableId::UNUSED;
            }
            self.tables
                .push_back(AcpiTable::new(id, dt as *const c_void, unsafe {
                    (*dt).length
                }));
        }
        self.number_of_tables = num_tables as u8;
        success!(module, "finished initialization\n");
    }

    pub fn new(rsdp: *const c_void, system_table: &mut SystemTable) -> AcpiTables {
        let mut tables = AcpiTables::default();
        tables.initialize(system_table, rsdp);
        return tables;
    }

    pub fn get_table(&self, table_id: AcpiTableId) -> Option<*const AcpiTable> {
        let mut __table: *const AcpiTable = null();

        let operand = |_: u32, ptr: *const AcpiTable| -> bool {
            if unsafe { (*ptr).table.clone() } == table_id {
                __table = ptr;
                return false;
            }
            return true;
        };
        self.tables.for_each(operand);
        if __table.is_null() {
            return Option::None;
        }
        return Option::Some(__table);
    }
}

impl Default for AcpiTables {
    fn default() -> Self {
        return Self {
            number_of_tables: 0,
            tables: PageAllocator::default(),
        };
    }
}
