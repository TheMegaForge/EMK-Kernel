use core::{
    arch::asm,
    ffi::{CStr, c_uchar, c_void},
    mem::offset_of,
    ptr::{null, read_unaligned},
    slice,
};

#[repr(C)]
pub struct ImageAllocation {
    allocated: u64,
    phdr: u64,
    size: u64,
}

use crate::{
    Pager,
    acpi_tables::{AcpiTable, AcpiTableId, AcpiTables, DSDT, FADT, descriptor_table},
    aml::{
        AmlCode, AmlError, AmlValue,
        definitions::{TermArg::NameString, TermArgInt},
        utils::{parse_data_ref_object, parse_name_string, parse_termarg, parse_termarg_int},
    },
    arch::{
        apic::{Apic, IO_APIC_ACTIVE_HIGH, IO_APIC_LEVEL},
        lapic::{DESTINATION_MODE_PHYSICAL, LocalApic},
    },
    drivers::{
        disk::{DiskController, nvme::NVMeController},
        hpet::Hpet,
        keyboard::Keyboard,
        usb::{Usb, independent::UsbHidDeviceType},
    },
    error,
    fixed_vaddrs::{
        DSDT_TABLE_FIXED_VADDR, FIXED_KERNEL_SPACE_MEMORY_VADDR, FIXED_LOCAL_APIC_VIRTUAL_ADDRESS,
        FIXED_PROCESSOR_VIRTUAL_ADDRESS, LOADER_NT64_RESOURCES_DLL_DATA_FIXED_VADDR,
        LOADER_NT64_RESOURCES_MAPPINGS_ARRAY_FIXED_VADDR,
        LOADER_NT64_RESOURCES_STRING_DATA_FIXED_VADDR, LOADER_RESOURCES_FIXED_VADDR,
        ref_processor_mut,
    },
    hal::{
        memory::{
            allocator::{
                Allocator, BITMAP_SIZE_IN_BYTES, BITMAP_SIZE_IN_PAGES,
                KERNEL_MAXIMUM_ALLOCATABLE_ADDRESS, MemoryBlock, VirtualAllocator,
            },
            pager::{PAGER_PCD, PAGER_PRESENT, PAGER_RW, Page},
        },
        pci_bus::PciBus,
        print::{GopFramebuffer, Module, print_init, simple_kernel_panic},
    },
    info,
    multithreading::{Multithreading, local_apic_spurious_interrupt, processors::Processor},
    processes::loader::{LoaderResources, NT64LoaderResources},
    success,
    utils::{Errno, memory::MemoryResult, reader::BufferedReader, slices::invalid_slice},
    vfs::gfs::{GFS, GeneralFileSystem},
};
pub struct PciRoutingInterrupt {
    pub irq: u8,
    /** 1 => edge, 0 => level*/
    pub trigger_mode: u8,
    /** 1 => low, 0 => high*/
    pub polarity: u8,
}

pub struct PciRoutingTable {
    base: *const c_void,
    num_entries: u16,
    path_root: u16,
}

impl PciRoutingTable {
    /** For APIC mode*/
    pub fn find_pci_int(
        &self,
        aml_code: &mut AmlCode,
        device: u8,
        func: u8,
        pin: u8,
    ) -> Option<PciRoutingInterrupt> {
        let specific = ((device as u32) << 16) | func as u32;

        let mut reader = BufferedReader::new(self.base, 0xFFFFFFFF);
        for i in 0..self.num_entries as usize {
            let (pkg, pkg_len) = match parse_data_ref_object(aml_code, &mut reader) {
                crate::aml::definitions::DataRefObject::Package(ptr, length, _) => (ptr, length),
                _ => simple_kernel_panic("PciRoutingTable", "Invalid Type in Routing Type\n"),
            };
            let mut pkg_reader = BufferedReader::new(pkg, pkg_len as u32);
            let addr;
            match parse_termarg_int(&mut pkg_reader) {
                crate::aml::definitions::TermArgInt::Word(val) => addr = val as u32,
                crate::aml::definitions::TermArgInt::DWord(val) => addr = val as u32,
                _ => simple_kernel_panic("PciRoutingTable", "Invalid Address Type\n"),
            }

            if addr == specific || ((addr & 0xFFFF) == 0xFFFF && (addr >> 16) == device as u32) {
                let pkg_pin = parse_termarg_int(&mut pkg_reader); // skips pin
                match pkg_pin {
                    TermArgInt::Zero => {
                        if pin != 0 {
                            continue;
                        }
                    }
                    TermArgInt::One => {
                        if pin != 1 {
                            continue;
                        }
                    }
                    TermArgInt::Byte(b) => {
                        if b != pin {
                            continue;
                        }
                    }
                    _ => simple_kernel_panic("PciRoutingTable", "Invalid Pin Type\n"),
                }

                let source = parse_termarg_int(&mut pkg_reader);

                if let TermArgInt::NameString(name_string) = source {
                    let path_system;
                    let str = name_string.as_str();

                    if name_string.is_absolute() {
                        path_system = aml_code.find_path_system(str, 0).unwrap();
                    } else {
                        path_system = aml_code.find_path_system(str, self.path_root).unwrap();
                    }
                    let device = aml_code.get_original_device(path_system).unwrap();
                    let ns = match device.get_name_system() {
                        Some(ns) => *ns,
                        None => simple_kernel_panic(
                            "PciRoutingTable",
                            "Could not get name system for device\n",
                        ),
                    };
                    match aml_code.get_content_of_name_system(
                        ns,
                        [
                            '_' as c_uchar,
                            'C' as c_uchar,
                            'R' as c_uchar,
                            'S' as c_uchar,
                        ],
                    ) {
                        Some(dro) => match dro {
                            crate::aml::definitions::DataRefObject::Buffer(ptr, len) => {
                                let source_index = match parse_termarg_int(&mut pkg_reader) {
                                    TermArgInt::Byte(b) => b as u32,
                                    TermArgInt::Word(w) => w as u32,
                                    TermArgInt::DWord(dw) => dw,
                                    TermArgInt::Zero => 0,
                                    TermArgInt::One => 1,
                                    _ => simple_kernel_panic(
                                        "PciRoutingTable",
                                        "Invalid Source Index type\n",
                                    ),
                                };

                                let mut rem = *len;
                                let mut cptr = *ptr;
                                while rem != 0 {
                                    let r#type = unsafe { (cptr as *const u8).read_unaligned() };
                                    if r#type & 0x80 == 0 {
                                        let length = r#type & 0x7;
                                        if (r#type >> 3) == 0x04 {
                                            let mask = unsafe {
                                                (cptr.add(1) as *const u16).read_unaligned()
                                            };

                                            let mut counter = 0;
                                            for i in 0..16 {
                                                if mask & (1 << i) != 0 {
                                                    if counter == source_index {
                                                        let flags = unsafe {
                                                            (cptr.add(3) as *const u8)
                                                                .read_unaligned()
                                                        };

                                                        return Option::Some(PciRoutingInterrupt {
                                                            irq: i,
                                                            trigger_mode: flags & 1,
                                                            polarity: (flags >> 3) & 1,
                                                        });
                                                    }
                                                    counter += 1;
                                                }
                                            }
                                        }
                                        cptr = unsafe { cptr.add(length as usize + 1) };
                                        rem -= length as u32 + 1;
                                    } else {
                                        assert_eq!(source_index, 0);
                                        if r#type & (0x80 - 1) == 0x9 {
                                            let flags = unsafe {
                                                (cptr.add(3) as *const u8).read_unaligned()
                                            };
                                            let interrupt = unsafe {
                                                (cptr.add(5) as *const u8).read_unaligned()
                                            };

                                            return Option::Some(PciRoutingInterrupt {
                                                irq: interrupt,
                                                trigger_mode: (flags >> 1) & 1,
                                                polarity: (flags >> 2) & 1,
                                            });
                                        }
                                        let length =
                                            unsafe { (cptr.add(1) as *const u16).read_unaligned() };
                                        rem -= length as u32;
                                        cptr = unsafe { cptr.add(length as usize) };
                                    }
                                }
                            }
                            _ => simple_kernel_panic("PciRoutingTable", "Invalid Source Type\n"),
                        },
                        None => {
                            match aml_code.execute_method_direct_path(
                                path_system,
                                c"_CRS",
                                invalid_slice(),
                            ) {
                                Ok(value) => {
                                    todo!()
                                }
                                Err(e) => {
                                    if let AmlError::NotFound = e {
                                        simple_kernel_panic(
                                            "PciRoutingTable",
                                            "Could not find _CRS method\n",
                                        );
                                    } else {
                                        simple_kernel_panic(
                                            "PciRoutingTable",
                                            "Could not execute _CRS\n",
                                        )
                                    }
                                }
                            }
                        }
                    }
                } else {
                    let source_index = parse_termarg_int(&mut pkg_reader);
                    todo!("Allocating from Global Interrupts\n");
                }
            }
        }
        return Option::None;
    }
}

pub const PAGES_FOR_32MB: u16 = 8192;

pub mod exceptions;
pub mod memory;
pub mod pci_bus;
pub mod print;
pub struct SystemTable {
    pub(super) virtual_allocator: VirtualAllocator,
    pub(super) physical_allocator: Allocator,
    pub(super) root_allocator: Allocator,
    acpi_tables: AcpiTables,
    pci_bus: PciBus,
    processor: *mut Processor,
    multithreading: Multithreading,
    apic: Apic,
    _hpet: Hpet,
    usb: Usb,
    aml_code: AmlCode,
    keyboard: Keyboard,
    pci_routing_table: PciRoutingTable,
    nvme_controller: NVMeController,
}

impl SystemTable {
    pub fn discover_acpi_tables(&mut self, rsdp: *const c_void) {
        let acpi_tables = AcpiTables::new(rsdp, self);
        self.acpi_tables = acpi_tables;
    }

    pub fn get_acpi_table(&self, table_id: AcpiTableId) -> Option<*const AcpiTable> {
        return self.acpi_tables.get_table(table_id);
    }

    pub fn discover_pci_devices(&mut self) {
        let pci_bus = PciBus::new(self);
        self.pci_bus = pci_bus;
    }

    pub fn initialize_multithreading(&mut self) {
        let madt_table = match self.get_acpi_table(AcpiTableId::APIC) {
            Some(table) => table,
            None => simple_kernel_panic(
                "SystemTable/initialize_multithreading",
                "'APIC' ACPI Table is missing\n",
            ),
        };

        let pager: &mut Pager = match unsafe { (*self.processor).pager.as_mut() } {
            Some(pager) => pager,
            None => {
                let mut module = Module::new("Kernel");
                error!(module, "Pager on Core {} is null\n", unsafe {
                    (*self.processor).get_activation_id()
                });
                Processor::halt_processor()
            }
        };
        self.multithreading = Multithreading::new(
            pager,
            &mut self.virtual_allocator.allocator,
            &mut self.physical_allocator,
            madt_table,
        );
        unsafe {
            (*self.processor).get_lapic().reset();
            // 223 corrosponds to isr255 (0xFF)
            (*self.processor).install_isr(local_apic_spurious_interrupt, 255);
        };
    }

    pub fn initialize_apic(&mut self) {
        let pager: &mut Pager = match unsafe { (*self.processor).pager.as_mut() } {
            Some(pager) => pager,
            None => {
                let mut module = Module::new("Kernel");
                error!(module, "Pager on Core {} is null\n", unsafe {
                    (*self.processor).get_activation_id()
                });
                Processor::halt_processor();
            }
        };

        let apic = Apic::new(
            self.multithreading.get_io_apics(),
            self.multithreading.get_gis_overrides(),
            pager,
            &mut self.physical_allocator,
            &mut self.virtual_allocator.allocator,
        );
        self.apic = apic;
    }

    pub fn initialize_hpet(&mut self) {
        let hpet_table = match self.acpi_tables.get_table(AcpiTableId::HPET) {
            Some(table) => table,
            None => simple_kernel_panic(
                "SystemTable/initialize_hpet",
                "'HPET' Acpi Table is missing\n",
            ),
        };

        let pager = match unsafe { (*self.processor).pager.as_mut() } {
            Some(pager) => pager,
            None => simple_kernel_panic("SystemTable/initialize_hpet", "Pager is null\n"),
        };
        let mut module = Module::new("Hpet");
        let mut hpet = Hpet::new(
            hpet_table,
            pager,
            &mut self.physical_allocator,
            &mut self.apic,
        );
        hpet.reset_fully();
        hpet.enable_all();
        success!(module, "Initialized & Activated\n");
    }

    pub fn enable_interrupts(&self) {
        unsafe { asm!("sti") }
        let mut module = Module::new("Interrupts");
        success!(module, "Enabled\n");
        unsafe {
            (*self.processor).get_lapic().send_eoi();
        }
    }

    pub fn initialize_cores(
        &mut self,
        num_allocations: u32,
        image_allocations: *const ImageAllocation,
    ) {
        let physical_allocator = &mut self.physical_allocator;
        let virtual_allocator = &mut self.virtual_allocator;
        self.multithreading.activate_cores(
            physical_allocator,
            &mut self.root_allocator,
            virtual_allocator,
            num_allocations,
            image_allocations as *const c_void,
        );
    }

    pub fn initialize_aml(&mut self) {
        let mut module = Module::new("Acpi/Machine Code");
        let fadt: &FADT = match self.acpi_tables.get_table(AcpiTableId::FACP) {
            Some(table) => unsafe { ((*table).get_ptr() as *const FADT).as_ref().unwrap() },
            None => simple_kernel_panic(
                "SystemTable/initialize_aml",
                "ACPI Table 'FACP' is missing\n",
            ),
        };
        let dsdt_table: &DSDT = unsafe { (fadt.dsdt as *const DSDT).as_ref().unwrap() };
        let bytes_of_aml = dsdt_table.header.get_length() as usize - size_of::<descriptor_table>();
        info!(module, "{} Bytes of Code in 'DSDT'\n", bytes_of_aml);
        let code_begin =
            dsdt_table as *const DSDT as u64 + offset_of!(DSDT, definition_table) as u64;
        info!(
            module,
            "(Physical) Begin of code = 0x{:x} ; Begin of Table = 0x{:x}\n",
            code_begin,
            dsdt_table as *const DSDT as u64
        );

        let mut pages = (bytes_of_aml + size_of::<descriptor_table>()) / 0x1000;
        if (bytes_of_aml + size_of::<descriptor_table>()) % 0x1000 != 0 {
            pages += 1;
        }
        let pager = ref_processor_mut().ref_mut_pager();
        for i in 0..pages as u64 {
            match pager.page_4_kb(
                DSDT_TABLE_FIXED_VADDR + i * 0x1000,
                (dsdt_table as *const DSDT as u64 + i * 0x1000),
                PAGER_PRESENT | PAGER_RW,
                &mut self.physical_allocator,
            ) {
                Ok(_) => {}
                Err(_e) => {
                    simple_kernel_panic("SystemTable/initialize_aml", "Cannot map dsdt table\n")
                }
            }
        }

        let aml_code = AmlCode::new(
            bytes_of_aml as u64,
            DSDT_TABLE_FIXED_VADDR + (code_begin - dsdt_table as *const DSDT as u64),
            &mut self.virtual_allocator.allocator,
        );
        self.aml_code = aml_code;
        success!(module, "Parsed\n");
    }

    pub fn route_interrupts(&mut self) {
        let mut module = Module::new("Acpi/route_interrupts");
        let arguments = [AmlValue::UnsignedNumber(1)];
        let no_arguments = [];
        match self.aml_code.execute_method(c"", c"_PIC", &arguments) {
            Ok(_) => {}
            Err(_e) => {
                simple_kernel_panic("SystemTable/route_interrupts", "Could not execute _PIC\n")
            }
        };
        let (routing_table_base, routing_table_size) =
            match self
                .aml_code
                .execute_method(c"_SB_PCI0", c"_PRT", &no_arguments)
            {
                Ok(ret_val) => match ret_val {
                    AmlValue::Package(base, entries) => (base, entries),
                    _ => simple_kernel_panic(
                        "SystemTable/route_interrupts",
                        "Invalid return from _SB.PCI0._PRT\n",
                    ),
                },
                Err(_e) => simple_kernel_panic(
                    "SystemTable/route_interrupts",
                    "Could not execute _SB.PCI0._PRT\n",
                ),
            };
        let path_system = self.aml_code.find_path_system("_SB_", 0).unwrap();
        self.pci_routing_table = PciRoutingTable {
            base: routing_table_base,
            num_entries: routing_table_size,
            path_root: path_system,
        };
        success!(&mut module, "Got Routing Table\n");
    }

    pub fn initialize_usb(&mut self) {
        self.usb = Usb::new(
            &self.pci_bus,
            &mut self.physical_allocator,
            &mut self.apic,
            &self.pci_routing_table,
            &mut self.aml_code,
        );
    }

    pub fn unpage_4k(&mut self, virt: u64) -> Option<MemoryResult> {
        return unsafe { (*((*self.processor).pager)).unpage_4k(virt) };
    }

    pub fn page_4k(&mut self, virt: u64, physical: u64, flags: u16) -> Result<Page, MemoryResult> {
        let allocator = &mut self.physical_allocator;
        return unsafe { (*((*self.processor).pager)).page_4_kb(virt, physical, flags, allocator) };
    }

    pub fn initialize_keyboard(&mut self) {
        let allocator = &mut self.physical_allocator;
        if let Option::Some((controller, device, hid)) =
            self.usb.find_hid_device(UsbHidDeviceType::Keyboard)
        {
            self.keyboard = Keyboard::new_usb(
                allocator,
                self.usb.get_mut_controller(controller).unwrap(),
                device,
                hid,
            );
        } else {
            simple_kernel_panic(
                "SystemTable/initialize_keyboard",
                "Unimplemented Check for PS2 Keyboard\n",
            );
        }
    }

    pub fn initialize_disk_drivers(&mut self) {
        let mut module = Module::new("Disk Drivers");
        /*
        let sata_controller = match self.pci_bus.find_pci_device(0x1, 0x6, 0x1) {
            Some(pci_device) => {
                let (bus, device, func) = PciBus::decompress_ident(pci_device);
                info!(
                    &mut module,
                    "Found Sata Controller at Bus {} Device {} Function {}\n", bus, device, func
                );
                let pager = unsafe {
                    (*(FIXED_PROCESSOR_VIRTUAL_ADDRESS as *mut Processor)).ref_mut_pager()
                };
                SataController::new(&self.pci_bus, pci_device, &mut self.allocator, pager)
            }
            None => SataController::not_present(),
        };
        TODO: initialize NVMe
        */
        match self.pci_bus.find_pci_device(0x2, 0x8, 0x1) {
            Some(pci_device) => {
                let (bus, device, func) = PciBus::decompress_ident(pci_device);
                info!(
                    &mut module,
                    "Found NVMe Controller at Bus {} Device {} Function {}\n", bus, device, func
                );
                let pager = unsafe {
                    (*(FIXED_PROCESSOR_VIRTUAL_ADDRESS as *mut Processor)).ref_mut_pager()
                };

                let isr_vector = unsafe {
                    (*(FIXED_PROCESSOR_VIRTUAL_ADDRESS as *mut Processor)).request_isr_vector()
                };

                let mut pin = self.pci_bus.get_pin(pci_device);
                assert_ne!(pin, 0);
                pin -= 1;

                let gsi =
                    match self
                        .pci_routing_table
                        .find_pci_int(&mut self.aml_code, device, func, pin)
                    {
                        Some(gsi) => gsi,
                        None => {
                            simple_kernel_panic(module.name(), "Could not get gsi for NVMe\n");
                        }
                    };
                info!(&mut module, "NVMe uses apic-irq {}\n", gsi.irq);
                self.apic.write_gsi(
                    gsi.irq as u32,
                    isr_vector,
                    DESTINATION_MODE_PHYSICAL,
                    gsi.polarity,
                    (!gsi.trigger_mode) & 1,
                    LocalApic::from_local_core().get_id(),
                );

                NVMeController::new(
                    &self.pci_bus,
                    pci_device,
                    &mut self.physical_allocator,
                    pager,
                    isr_vector,
                    &mut self.nvme_controller,
                );

                self.multithreading.foreach_lapic(|lapic| {
                    if lapic.get_id() as u32 != LocalApic::from_local_core().get_id() {
                        self.nvme_controller
                            .create_io_pair(&mut self.physical_allocator); // schedulers
                    }
                });
                self.nvme_controller.sync_vmem(&self.multithreading);
            }
            None => {}
        };
        success!(&mut module, "Initialized\n");
    }
    pub fn initialize_file_systems(&mut self) {
        let nvme_controller;
        if self.nvme_controller.present() {
            nvme_controller = Option::Some(&self.nvme_controller);
        } else {
            nvme_controller = Option::None;
        }
        unsafe {
            GFS =
                GeneralFileSystem::new(nvme_controller, Option::None, &mut self.physical_allocator);
            #[allow(static_mut_refs)]
            GFS.sync(&self.multithreading);
        };
    }

    pub fn initialize_executable_loader(&mut self) -> &'static mut LoaderResources {
        if size_of::<LoaderResources>() > 0x1000 {
            simple_kernel_panic(
                "SystemTable/initialize_executable_loader",
                "LoaderResources take up more than 1 page of memory\n",
            );
        }

        let loader_physical_memory = self.physical_allocator.alloc_zero(1).unwrap();
        let pager = ref_processor_mut().ref_mut_pager();

        let loader_resources =
            unsafe { &mut *(loader_physical_memory.base as *mut LoaderResources) };
        *loader_resources = LoaderResources::new(&mut self.physical_allocator, pager);

        let mut memory_to_map =
            loader_resources.ref_nt64().memory[NT64LoaderResources::MAPPING_MEMORY_INDEX];
        memory_to_map.map(
            &mut self.physical_allocator,
            pager,
            LOADER_NT64_RESOURCES_MAPPINGS_ARRAY_FIXED_VADDR,
            PAGER_RW | PAGER_PRESENT,
        );
        memory_to_map =
            loader_resources.ref_nt64().memory[NT64LoaderResources::STRING_MEMORY_INDEX];
        memory_to_map.map(
            &mut self.physical_allocator,
            pager,
            LOADER_NT64_RESOURCES_STRING_DATA_FIXED_VADDR,
            PAGER_RW | PAGER_PRESENT,
        );

        self.multithreading.foreach_lapic(|lapic| {
            if lapic.get_id() as u32 != LocalApic::from_local_core().get_id() {
                self.multithreading.send_user_ipi(lapic.get_id(), |packet| {
                    packet.request_type = crate::processes::IpiRequestType::SyncVMem;
                    packet.status = crate::processes::IpiStatus::Pending;
                    packet.request_data.vmem_sync.vaddr = LOADER_RESOURCES_FIXED_VADDR;
                    packet.request_data.vmem_sync.paddr = loader_physical_memory;
                });
                if !self.multithreading.await_ipi(lapic.get_id()) {
                    simple_kernel_panic(
                        "SystemTable/initialize_executable_loader",
                        "ipi got stuck\n",
                    );
                }

                self.multithreading.send_user_ipi(lapic.get_id(), |packet| {
                    packet.request_type = crate::processes::IpiRequestType::SyncVMem;
                    packet.status = crate::processes::IpiStatus::Pending;
                    packet.request_data.vmem_sync.vaddr =
                        LOADER_NT64_RESOURCES_MAPPINGS_ARRAY_FIXED_VADDR;
                    packet.request_data.vmem_sync.paddr = loader_resources.ref_nt64().memory
                        [NT64LoaderResources::MAPPING_MEMORY_INDEX];
                });
                if !self.multithreading.await_ipi(lapic.get_id()) {
                    simple_kernel_panic(
                        "SystemTable/initialize_executable_loader",
                        "ipi got stuck\n",
                    );
                }
                self.multithreading.send_user_ipi(lapic.get_id(), |packet| {
                    packet.request_type = crate::processes::IpiRequestType::SyncVMem;
                    packet.status = crate::processes::IpiStatus::Pending;
                    packet.request_data.vmem_sync.vaddr =
                        LOADER_NT64_RESOURCES_STRING_DATA_FIXED_VADDR;
                    packet.request_data.vmem_sync.paddr = loader_resources.ref_nt64().memory
                        [NT64LoaderResources::STRING_MEMORY_INDEX];
                });

                if !self.multithreading.await_ipi(lapic.get_id()) {
                    simple_kernel_panic(
                        "SystemTable/initialize_executable_loader",
                        "ipi got stuck\n",
                    );
                }
            }
        });

        return loader_resources;
    }

    pub fn load_system_libraries(&mut self, resources: &mut LoaderResources) {
        resources
            .load_nt64_system_libraries(&mut self.virtual_allocator, &mut self.physical_allocator);
    }

    pub fn test(&self) {
        self.multithreading.send_user_ipi(1, |packet| {
            packet.status = crate::processes::IpiStatus::Pending;
            packet.request_type = crate::processes::IpiRequestType::SummonApplication;
            packet.request_data.application_to_summon = "z:\\test001.exe";
        });
        if !self.multithreading.await_ipi(1) {
            simple_kernel_panic("test", "ipi got stuck\n");
        }
    }
}

unsafe extern "C" {
    static __kend: u8;
}

fn __kend_ptr() -> *const c_void {
    return core::ptr::addr_of!(__kend) as *const c_void;
}

//TODO: update this! + extract
#[unsafe(no_mangle)]
pub fn hal_init(
    tmp_buffer_address: *mut c_void,
    framebuffer: *mut GopFramebuffer,
    font: *const c_void,
    descriptor_size: u32,
    descriptor_count: u32,
    memory_descriptor: *mut c_void,
    num_allocations: u32,
    image_allocations: *const c_void,
) -> Result<SystemTable, Errno> {
    unsafe { print_init(tmp_buffer_address, framebuffer, font) };

    let mut allocator: Allocator;

    let mut hal_mod: Module = Module::new("HAL");
    info!(hal_mod, "Began Initialization\n");
    info!(
        hal_mod,
        "Pages needed for Bitmap {} => {} Bytes ; highest address 0x{:x}\n",
        BITMAP_SIZE_IN_PAGES,
        BITMAP_SIZE_IN_BYTES,
        KERNEL_MAXIMUM_ALLOCATABLE_ADDRESS
    );
    info!(hal_mod, "Kernel End = 0x{:x}\n", __kend_ptr() as u64);
    info!(hal_mod, "Framebuffer address: {:x}\n", unsafe {
        (*framebuffer).get_base()
    });
    allocator = match Allocator::new(
        &mut hal_mod,
        descriptor_size,
        descriptor_count,
        memory_descriptor,
        num_allocations,
        image_allocations,
    ) {
        Ok(allocator) => allocator,
        Err(e) => {
            return Err(e);
        }
    };

    let physical_allocator = allocator.subdivide(PAGES_FOR_32MB);
    let virtual_allocator = allocator.new_fake(PAGES_FOR_32MB, FIXED_KERNEL_SPACE_MEMORY_VADDR);

    let virtual_allocator_physical_equivalent = match allocator.alloc(PAGES_FOR_32MB) {
        Ok(mb) => mb,
        Err(_e) => simple_kernel_panic("hal_init", "Could not allocate memory for kernelspace\n"),
    };

    success!(hal_mod, "Completed HAL Initialization\n");
    return Ok(SystemTable {
        root_allocator: allocator,
        virtual_allocator: VirtualAllocator::new(
            virtual_allocator_physical_equivalent,
            virtual_allocator,
        ),
        physical_allocator,
        acpi_tables: AcpiTables::default(),
        pci_bus: PciBus::default(),
        processor: FIXED_PROCESSOR_VIRTUAL_ADDRESS as *mut Processor,
        multithreading: Multithreading::default(),
        apic: Apic::default(),
        _hpet: Hpet::default(),
        usb: Usb::default(),
        aml_code: AmlCode::default(),
        keyboard: Keyboard::default(),
        pci_routing_table: PciRoutingTable {
            base: null(),
            num_entries: 0,
            path_root: 0,
        },
        nvme_controller: NVMeController::not_present(),
    });
}
