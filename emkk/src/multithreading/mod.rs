use core::{
    ffi::c_void,
    mem::offset_of,
    ptr::{self, addr_of},
    slice,
};

use crate::{
    acpi_tables::{AcpiTable, descriptor_table},
    arch::{
        gdt::GdtDescriptor,
        isr::ISRRegisters,
        lapic::{
            DELIVERY_MODE_FIXED, DELIVERY_MODE_INIT, DESTINATION_MODE_PHYSICAL, LEVEL_ASSERT,
            LEVEL_DE_ASSERT, LocalApic, NO_SHORTHAND, TRIGGER_MODE_EDGE, TRIGGER_MODE_LEVEL,
        },
    },
    fixed_vaddrs::{
        APPLICATION_CORE_TSS_FIXED_VADDR, APPLICATION_CORE_TSS_RSP0_FIXED_VADDR_END,
        FIXED_PROCESSOR_VIRTUAL_ADDRESS, IO_APIC_FIXED_VADDR, ref_processor_mut,
    },
    hal::{
        memory::{
            allocator::{Allocator, MemoryBlock, VirtualAllocator},
            pager::{PAGER_PCD, PAGER_PRESENT, PAGER_RW, Pager},
        },
        print::{Module, page_framebuffer_virtual, simple_kernel_panic},
    },
    info,
    multithreading::{per_processor_function::ap_function, processors::Processor},
    processes::IpiDescriptorPacket,
    time::sleep,
    utils::{
        allocators::DynamicAllocator, invalid_mut_slice, list::List, memory::memcpy, traits::Region,
    },
};

pub mod per_processor_function;
pub mod processors;

#[allow(dead_code)]
pub struct MADT {
    header: descriptor_table,
    local_interrupt_controller_address: u32,
    flags: u32,
    interrupt_controller_structures: [c_void; 0],
}

pub struct IoApic {
    pub physical_address: u64,
    pub virtual_address: u64,
    pub gis_base: u32,
    pub ioapic_id: u8,
}

impl IoApic {
    pub fn write_register(&self, offset: u8, val: u32) {
        unsafe {
            /* tell IOREGSEL where we want to write to */
            (self.virtual_address as *mut u32).write_volatile(offset as u32);
            /* write the value to IOWIN */
            ((self.virtual_address + 0x10) as *mut u32).write_volatile(val);
        }
    }

    pub fn read_register(&self, offset: u8) -> u32 {
        unsafe {
            /* tell IOREGSEL where we want to read from */
            (self.virtual_address as *mut u32).write_volatile(offset as u32);
            /* return the data from IOWIN */
            return ((self.virtual_address + 0x10) as *const u32).read_volatile();
        }
    }
}

#[allow(dead_code)]
pub struct GsiOverride {
    pub io_apic_redirection_entry: u32,
    pub gsi: u32,
    flags: u16,
    pub corresponding_io_apic_index: u16,
    bus: u8,
    src: u8,
    pub registered: bool,
}

impl GsiOverride {
    pub fn new(
        io_apic_redirection_entry: u32,
        gsi: u32,
        flags: u16,
        corresponding_io_apic_index: u16,
        bus: u8,
        src: u8,
        registered: bool,
    ) -> Self {
        return Self {
            io_apic_redirection_entry,
            gsi,
            flags,
            corresponding_io_apic_index,
            bus,
            src,
            registered,
        };
    }
}

impl IoApic {
    pub fn new(physical_address: u64, virtual_address: u64, gis_base: u32, ioapic_id: u8) -> Self {
        return Self {
            physical_address,
            gis_base,
            ioapic_id,
            virtual_address,
        };
    }
}
pub struct MultithreadingLocalApic {
    flags: u32,
    acpi_processor_id: u8,
    apic_id: u8,
}

impl MultithreadingLocalApic {
    pub fn new(flags: u32, acpi_processor_id: u8, apic_id: u8) -> Self {
        return Self {
            flags,
            acpi_processor_id,
            apic_id,
        };
    }
    pub fn get_id(&self) -> u8 {
        return self.apic_id;
    }
}

pub struct Multithreading {
    memory: MemoryBlock,
    io_apics: List<IoApic>,
    gis_overrides: List<GsiOverride>,
    local_apics: List<MultithreadingLocalApic>,
    application_complex_paddrs: &'static mut [(u8, u64)],
    ipi_descriptor_packet_padds: &'static mut [(u8, u64)],
}

impl Default for Multithreading {
    fn default() -> Self {
        return Self {
            io_apics: List::default(),
            gis_overrides: List::default(),
            local_apics: List::default(),
            memory: MemoryBlock::default(),
            application_complex_paddrs: invalid_mut_slice(),
            ipi_descriptor_packet_padds: invalid_mut_slice(),
        };
    }
}

pub fn local_apic_spurious_interrupt(_registers: &ISRRegisters) {}

unsafe extern "C" {
    fn _ap_trampoline();
    static _ap_trampoline_end: u8;
    fn _ap_bootup0();
    static _ap_bootup0_end: u8;
    fn _ap_bootup1();
    static _ap_bootup1_end: u8;
}

impl Multithreading {
    pub fn get_io_apics(&self) -> *const List<IoApic> {
        return &raw const self.io_apics;
    }

    pub fn get_gis_overrides(&mut self) -> *mut List<GsiOverride> {
        return &raw mut self.gis_overrides;
    }

    pub fn foreach_lapic(&self, mut func: impl FnMut(&MultithreadingLocalApic)) {
        for i in 0..self.local_apics.entries {
            (func)(unsafe { self.local_apics.ptr.add(i as usize).as_mut().unwrap() });
        }
    }

    pub fn send_user_ipi(
        &self,
        apic_id: u8,
        mut prepare_func: impl FnMut(&mut IpiDescriptorPacket),
    ) {
        for (corrosponding_apic_id, paddr) in &*self.ipi_descriptor_packet_padds {
            if *corrosponding_apic_id == apic_id {
                (prepare_func)(unsafe { (*paddr as *mut IpiDescriptorPacket).as_mut().unwrap() });
                LocalApic::from_local_core().send_ipi(
                    apic_id,
                    254,
                    DELIVERY_MODE_FIXED,
                    DESTINATION_MODE_PHYSICAL,
                    LEVEL_ASSERT,
                    TRIGGER_MODE_EDGE,
                    NO_SHORTHAND,
                );
                return;
            }
        }
        simple_kernel_panic(
            "Multithreading/send_user_ipi",
            "Could not find corrosponding apic\n",
        );
    }

    pub fn await_ipi(&self, apic_id: u8) -> bool {
        for (corrosponding_apic_id, paddr) in &*self.ipi_descriptor_packet_padds {
            if *corrosponding_apic_id == apic_id {
                let ipi = unsafe { (*paddr as *mut IpiDescriptorPacket).as_mut().unwrap() };
                let mut tries = 50;
                while tries != 0 {
                    if let crate::processes::IpiStatus::Completed = ipi.status {
                        return true;
                    }
                    sleep(10);
                    tries -= 1;
                }
                return false;
            }
        }
        simple_kernel_panic(
            "Multithreading/await_ipi",
            "Could not find corrosponding apic\n",
        );
    }

    pub fn activate_cores(
        &self,
        physical_allocator: &mut Allocator,
        kernel_allocator: &mut Allocator,
        virtual_allocator: &mut VirtualAllocator,
        num_allocations: u32,
        image_allocations: *const c_void,
    ) {
        let processor = ref_processor_mut();

        let trampoline_length = (unsafe {
            (addr_of!(_ap_trampoline_end) as *const c_void)
                .offset_from(_ap_trampoline as *const c_void)
        }) as u32;

        let bootup0_length = (unsafe {
            (addr_of!(_ap_bootup0_end) as *const c_void).offset_from(_ap_bootup0 as *const c_void)
        }) as u32;

        let bootup1_length = (unsafe {
            (addr_of!(_ap_bootup1_end) as *const c_void).offset_from(_ap_bootup1 as *const c_void)
        }) as u32;
        unsafe {
            memcpy(
                0x8000 as *mut c_void,
                _ap_trampoline as *const c_void,
                trampoline_length,
            );
            memcpy(
                0xA000 as *mut c_void,
                _ap_bootup0 as *const c_void,
                bootup0_length,
            );
            memcpy(
                0xA100 as *mut c_void,
                _ap_bootup1 as *const c_void,
                bootup1_length,
            );
        }

        let gdt_descriptor: *mut GdtDescriptor = 0x9080 as *mut GdtDescriptor;
        unsafe {
            (*gdt_descriptor) = GdtDescriptor::standard(0x9100);
        }
        unsafe {
            *(0x9008 as *mut u64) = 0x1000 * 32
                + match virtual_allocator.allocator.alloc_zero(32) {
                    Ok(mb) => mb.get_base(),
                    Err(_e) => simple_kernel_panic(
                        "Multithreading/activate_cores",
                        "Could not allocate stack\n",
                    ),
                };
            *(0x9010 as *mut u64) = ap_function as *const () as u64;
        };

        let mut activation_index: u8 = 1;

        self.local_apics.for_each(|index, local_apic| -> bool {
            if processor.get_lapic().get_id() == unsafe { (*local_apic).apic_id as u32 } {
                // core is BSP
                true
            } else {
                let stack_base = match physical_allocator.alloc_zero(32) {
                    Ok(mb) => mb.get_base(),
                    Err(_e) => simple_kernel_panic(
                        "Multithreading/activate_cores",
                        "Could not allocate stack\n",
                    ),
                };
                unsafe { *(0x9008 as *mut u64) = 0x1000 * 32 + stack_base }
                let cr3: *mut u64 = match physical_allocator.alloc_zero(1) {
                    Ok(mb) => mb.as_mut_ptr(),
                    Err(_e) => {
                        simple_kernel_panic(
                            "Multithreading/activate_cores",
                            "Could not allocate cr3\n",
                        );
                    }
                };
                if cr3 as u64 > 0xFFFFFFFF {
                    simple_kernel_panic(
                        "Multithreading/activate_cores",
                        "Address of allocated cr3 is higher than 0xFFFFFFFF\n",
                    );
                }

                //TODO: test this!
                let mut pager = Pager::new(cr3);
                match pager.page_general(physical_allocator) {
                    Some(_e) => simple_kernel_panic("Multithreading/activate_cores", "Could not page general\n"),
                    None => {},
                }

                match pager.page_kernel(physical_allocator, num_allocations, image_allocations) {
                    Some(_e) => simple_kernel_panic("Multithreading/activate_cores", "Could not page kernel\n"),
                    None => {},
                };

                page_framebuffer_virtual(&mut pager, physical_allocator);
                pager.page_kernelspace_memory(physical_allocator, virtual_allocator);

                match pager.page_stack(physical_allocator, stack_base as *const c_void) {
                    Some(_e) => simple_kernel_panic("Multithreading/activate_cores", "Could not page stack\n"),
                    None => {},
                };

                let processor_physical = match physical_allocator.alloc(1) {
                    Ok(mb) => mb.get_base(),
                    Err(_e) => {
                        simple_kernel_panic("Multithreading/activate_cores", "Could not allocate physical address for 'FIXED_PROCESSOR_VIRTUAL_ADDRESS'\n")
                    }
                };

                let tss_physical = match physical_allocator.alloc_zero(1) {
                    Ok(mb) => mb.get_base(),
                    Err(_e) => {
                        simple_kernel_panic("Multithreading/activate_cores", "Could not allocate physical address for 'APPLICATION_CORE_TSS_FIXED_VADDR'\n")
                    },
                };

                let tss_rsp0_stack = match physical_allocator.alloc_zero(16) {
                    Ok(tss_rsp0_stack) => tss_rsp0_stack.get_base(),
                    Err(_e) => {
                        simple_kernel_panic("Multithreading/activate_cores", "Could not allocate physical address for Application Core TSS rsp0\n")
                    },
                };

                for i in 0..16 {
                    pager.page_4_kb(APPLICATION_CORE_TSS_RSP0_FIXED_VADDR_END + i * 0x1000, tss_rsp0_stack + i* 0x1000, PAGER_RW | PAGER_PRESENT, physical_allocator).unwrap();
                }

                pager.page_4_kb(APPLICATION_CORE_TSS_FIXED_VADDR, tss_physical, PAGER_RW | PAGER_PRESENT, physical_allocator).unwrap();
                unsafe {*(0x9018 as *mut u64) = processor_physical}
                unsafe {*(0x9020 as *mut u8)  = activation_index};
                unsafe {*(0x9028 as *mut u64) = ptr::from_mut(physical_allocator) as u64};
                unsafe {*(0x9030 as *mut u64) = self.application_complex_paddrs[index as usize].1};
                unsafe {*(0x9038 as *mut u64) = self.ipi_descriptor_packet_padds[index as usize].1};
                unsafe {*(0x9040 as *mut u64) = ptr::from_mut(virtual_allocator) as u64}
                unsafe {*(0x9048 as *mut u64) = ptr::from_mut(kernel_allocator) as u64}
                activation_index+=1;
                match pager.page_4_kb(FIXED_PROCESSOR_VIRTUAL_ADDRESS, processor_physical, PAGER_PRESENT | PAGER_RW, physical_allocator) {
                    Ok(_) => {},
                    Err(_e) => {
                        simple_kernel_panic("Multithreading/activate_cores", "Could not page 'FIXED_PROCESSOR_VIRTUAL_ADDRESS'\n")
                    }
                }
                unsafe { *(0x9000 as *mut u32) = cr3 as u32 };
                processor.get_lapic().send_ipi(
                    unsafe { (*local_apic).apic_id },
                    0,
                    DELIVERY_MODE_INIT,
                    DESTINATION_MODE_PHYSICAL,
                    LEVEL_ASSERT,
                    TRIGGER_MODE_LEVEL,
                    NO_SHORTHAND,
                );
                sleep(20);
                processor.get_lapic().send_ipi(
                    unsafe { (*local_apic).apic_id },
                    0,
                    DELIVERY_MODE_INIT,
                    DESTINATION_MODE_PHYSICAL,
                    LEVEL_DE_ASSERT,
                    TRIGGER_MODE_LEVEL,
                    NO_SHORTHAND,
                );
                sleep(20);
                processor.get_lapic().send_startup_ipi(
                    unsafe { (*local_apic).apic_id },
                    0x8, // => 0x8000
                    DESTINATION_MODE_PHYSICAL,
                    NO_SHORTHAND,
                );
                sleep(200);

                true
            }
        });
    }

    pub fn new(
        pager: &mut Pager,
        virtual_allocator: &mut Allocator,
        physical_allocator: &mut Allocator,
        madt_table: *const AcpiTable,
    ) -> Self {
        let madt = unsafe { (*madt_table).get_ptr() } as *const MADT;

        let mut module = Module::new("Multithreading");

        let mut ioapics: DynamicAllocator<IoApic> = DynamicAllocator::new(virtual_allocator, 2);
        let mut gsi_overrides: DynamicAllocator<GsiOverride> =
            DynamicAllocator::new(virtual_allocator, 2);
        let mut local_apics: DynamicAllocator<MultithreadingLocalApic> =
            DynamicAllocator::new(virtual_allocator, 2);

        let mut bytes = unsafe { (*madt).header.get_length() - 44 };
        let mut entry_base = unsafe {
            (madt as *const c_void).add(offset_of!(MADT, interrupt_controller_structures))
        };
        while bytes > 0 {
            let entry_type = unsafe { *(entry_base as *const u8) };
            let length = unsafe { *(entry_base.add(1) as *const u8) };
            if entry_type == 1
            /*IO/APIC */
            {
                let ioapic_id = unsafe { (entry_base.add(2) as *const u8).read_unaligned() };
                let ioapic_address = unsafe { (entry_base.add(4) as *const u32).read_unaligned() };
                let gsib = unsafe { (entry_base.add(8) as *const u32).read_unaligned() };
                info!(
                    module,
                    "ioapic {}: address 0x{:x} gsi base 0x{:x}\n", ioapic_id, ioapic_address, gsib
                );
                let io_apic =
                    IoApic::new(ioapic_address as u64, IO_APIC_FIXED_VADDR, gsib, ioapic_id);
                match ioapics.push_back(virtual_allocator, io_apic) {
                    Some(_e) => simple_kernel_panic(
                        "Multithreading/IoApic",
                        "Could not add ioapic to list\n",
                    ),
                    None => {}
                }

                entry_base = unsafe { entry_base.add(length as usize) };
                bytes -= length as u32;
            } else if entry_type == 2
            /*GISOverride */
            {
                let bus = unsafe { (entry_base.add(2) as *const u8).read_unaligned() };
                let source = unsafe { (entry_base.add(3) as *const u8).read_unaligned() };
                let gsi = unsafe { (entry_base.add(4) as *const u32).read_unaligned() };
                let flags = unsafe { (entry_base.add(8) as *const u16).read_unaligned() };
                /*
                info!(
                    module,
                    "GSIOverride 0x{:x}: bus {} source {} flags {}\n", gsi, bus, source, flags
                );
                */
                for i in 0..ioapics.size() as u16 {
                    let ioapic: *const IoApic = match ioapics.ref_const(i as u32) {
                        Some(apic) => apic,
                        None => {
                            simple_kernel_panic(
                                "Multithreading/GSIOverride",
                                "Could not get IoApic\n",
                            );
                        }
                    };
                    if gsi >= unsafe { (*ioapic).gis_base }
                        && gsi <= unsafe { (*ioapic).gis_base + 24 }
                    {
                        let r#override = GsiOverride::new(
                            unsafe { gsi - (*ioapic).gis_base },
                            gsi,
                            flags,
                            i,
                            bus,
                            source,
                            false,
                        );

                        match gsi_overrides.push_back(virtual_allocator, r#override) {
                            Some(_e) => {
                                simple_kernel_panic(
                                    "Multithreading/GISOverride",
                                    "Could not add GISOverride to list\n",
                                );
                            }
                            None => {}
                        }

                        break;
                    }
                }
                entry_base = unsafe { entry_base.add(length as usize) };
                bytes -= length as u32;
            } else if entry_type == 0
            /*Local APIC*/
            {
                let acpi_processor_id =
                    unsafe { (entry_base.add(2) as *const u8).read_unaligned() };
                let apic_id = unsafe { (entry_base.add(3) as *const u8).read_unaligned() };
                let flags = unsafe { (entry_base.add(4) as *const u32).read_unaligned() };
                if local_apics.size() > 16 {
                    simple_kernel_panic("Multithreading/LocalApic", "More than 16 CPU Cores\n");
                }

                let local_apic = MultithreadingLocalApic::new(flags, acpi_processor_id, apic_id);
                match local_apics.push_back(virtual_allocator, local_apic) {
                    Some(_e) => {
                        simple_kernel_panic(
                            "Multithreading/LocalApic",
                            "Could not add local apic to list\n",
                        );
                    }
                    None => {}
                }
                info!(
                    module,
                    "Local Apic {}: acpi_processor_id {} flags {}\n",
                    apic_id,
                    acpi_processor_id,
                    flags
                );
                entry_base = unsafe { entry_base.add(length as usize) };
                bytes -= length as u32;
            } else {
                //printf_e9("--- Entry with type %d --- \n",type);
                entry_base = unsafe { entry_base.add(length as usize) };
                bytes -= length as u32;
            }
        }

        let local_apics_list = local_apics.to_list();
        let memory = physical_allocator.alloc_zero(1).unwrap();
        let ipi_descriptor_packet_paddrs = unsafe {
            slice::from_raw_parts_mut(
                memory.base as *mut (u8, u64),
                local_apics_list.entries as usize,
            )
        };
        let application_complex_paddrs = unsafe {
            slice::from_raw_parts_mut(
                (memory.base + local_apics_list.entries as u64 * size_of::<(u8, u64)>() as u64)
                    as *mut (u8, u64),
                local_apics_list.entries as usize,
            )
        };

        local_apics_list.for_each(|index, local_apic| -> bool {
            let lapic = unsafe { local_apic.as_ref().unwrap() };
            if lapic.apic_id as u32 == LocalApic::from_local_core().get_id() {
                ipi_descriptor_packet_paddrs[index as usize].0 = lapic.apic_id;
                application_complex_paddrs[index as usize].0 = lapic.apic_id;
                /* no allocation for kernel core*/
            } else {
                ipi_descriptor_packet_paddrs[index as usize].0 = lapic.apic_id;
                ipi_descriptor_packet_paddrs[index as usize].1 =
                    physical_allocator.alloc_zero(1).unwrap().base;
                application_complex_paddrs[index as usize].0 = lapic.apic_id;
                application_complex_paddrs[index as usize].1 =
                    physical_allocator.alloc_zero(1).unwrap().base;
            }
            true
        });
        return Self {
            io_apics: ioapics.to_list(),
            gis_overrides: gsi_overrides.to_list(),
            local_apics: local_apics_list,
            memory,
            ipi_descriptor_packet_padds: ipi_descriptor_packet_paddrs,
            application_complex_paddrs,
        };
    }
}
