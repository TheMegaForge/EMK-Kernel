use core::{ffi::c_void, ptr::addr_of};

use crate::{
    acpi_tables::descriptor_table,
    hal::{
        SystemTable,
        memory::{
            allocator::Allocator,
            pager::{PAGER_PCD, PAGER_PRESENT, PAGER_RW, Pager, page_align},
        },
        print::{Module, simple_kernel_panic},
    },
    info, success,
    utils::{allocators::DynamicAllocator, list::List, memory::memcpy_qword},
};

pub struct PciDevice {}

pub struct PciBus {
    pci_devices: List<u64>,
    __pci_base: u64,
}

pub enum PciBarIndex {
    Index0,
    Index1,
    Index2,
    Index3,
    Index4,
    Index5,
}
#[derive(PartialEq)]
pub enum BarType {
    Memory,
    Io,
}

pub struct PciBar {
    bar_type: BarType,
    #[allow(dead_code)]
    flags: u8,
    data: u64,
    length: u32,
}

impl PciBar {
    pub fn get_address(&self) -> u64 {
        return self.data;
    }

    pub fn get_length(&self) -> u32 {
        return self.length;
    }

    /*
     * returns true, when the bar was successfully mapped
     */
    pub fn map(&self, pager: &mut Pager, allocator: &mut Allocator) -> bool {
        if self.bar_type == BarType::Memory {
            for i in 0..self.length / 0x1000 {
                match pager.page_4_kb(
                    self.data + (i * 0x1000) as u64,
                    self.data + (i * 0x1000) as u64,
                    PAGER_PCD | PAGER_RW | PAGER_PRESENT,
                    allocator,
                ) {
                    Ok(_) => {}
                    Err(_e) => return false,
                }
            }
        }
        return true;
    }

    pub fn map_to_virtual(
        &self,
        pager: &mut Pager,
        map: u64,
        physical_allocator: &mut Allocator,
    ) -> bool {
        if self.bar_type == BarType::Memory {
            for i in 0..self.length / 0x1000 {
                match pager.page_4_kb(
                    map + (i * 0x1000) as u64,
                    self.data + (i * 0x1000) as u64,
                    PAGER_PCD | PAGER_RW | PAGER_PRESENT,
                    physical_allocator,
                ) {
                    Ok(_) => {}
                    Err(_e) => return false,
                }
            }
        }
        return true;
    }
}

impl Default for PciBus {
    fn default() -> Self {
        return Self {
            pci_devices: List::default(),
            __pci_base: 0,
        };
    }
}

#[repr(C)]
#[derive(Default)]
struct EnhancedConfigurationMechanism {
    base_address: u64,
    segment_group_number: u16,
    start_pci_bus_number: u8,
    end_pci_bus_number: u8,
    reserved: u32,
}

#[repr(C, packed)]
struct MCFG {
    header: descriptor_table,
    reserved: u64,
    entries: [EnhancedConfigurationMechanism; 0],
}

type Bus = u8;
type Device = u8;
type Func = u8;

pub const PCI_MEMORY_BAR_32BITS: u32 = 0;

impl PciBus {
    pub fn compress_ident(bus: u8, device: u8, func: u8) -> u64 {
        return func as u64 | (device as u64) << 8 | (bus as u64) << 16;
    }

    pub fn decompress_ident(pci_ident: u64) -> (Bus, Device, Func) {
        return (
            ((pci_ident >> 16) & 0xFF) as u8,
            ((pci_ident >> 8) & 0xFF) as u8,
            (pci_ident & 0xFF) as u8,
        );
    }

    pub fn pci_base(&self, pci_ident: u64) -> Option<*const c_void> {
        for i in 0..self.pci_devices.entries {
            match self.pci_devices.ref_const(i) {
                Some(_) => {
                    let (bus, device, func) = PciBus::decompress_ident(pci_ident);
                    let dev_desc = bus as u64 * 256 + device as u64 * 8 + func as u64;
                    return Option::Some((self.__pci_base + dev_desc * 0x1000) as *const c_void);
                }
                None => simple_kernel_panic("PciBus/pci_base", "Could not get pci_ident\n"),
            }
        }
        return Option::None;
    }

    pub fn find_pci_device(&self, prog_if: u8, subclass: u8, classcode: u8) -> Option<u64> {
        for i in 0..self.pci_devices.entries {
            let pci_device = match self.pci_devices.ref_const(i) {
                Some(pci_device) => pci_device,
                None => simple_kernel_panic("PciBus/find_pci_device", "Could not get pci_devicen"),
            };

            let base = match PciBus::pci_base(&self, *pci_device) {
                Some(base) => base,
                None => simple_kernel_panic("PciBus/find_pci_device", "Could not find device\n"),
            };

            let pci_prog_if = unsafe { (base.add(0x9) as *const u8).read_volatile() };
            let pci_subclass = unsafe { (base.add(0xA) as *const u8).read_volatile() };
            let pci_classcode = unsafe { (base.add(0xB) as *const u8).read_volatile() };

            if pci_prog_if != prog_if || pci_subclass != subclass || pci_classcode != classcode {
                continue;
            }
            return Option::Some(*pci_device);
        }
        return Option::None;
    }

    pub fn get_capability(&self, pci_ident: u64, capability: u8) -> Option<u8> {
        let pci_base = match self.pci_base(pci_ident) {
            Some(base) => base,
            None => return Option::None,
        };

        if unsafe { (pci_base.add(0x6) as *const u16).read_volatile() & (1 << 4) } == 0 {
            return Option::None;
        }

        let old_cmd: u16 = unsafe { (pci_base.add(4) as *const u16).read_volatile() };
        unsafe {
            (pci_base.add(4) as *mut u16)
                .write_volatile((pci_base.add(4) as *mut u16).read_volatile() & !7)
        };
        let cfg = pci_base as *const u8;

        let mut pos = unsafe { cfg.add(0x34).read_volatile() };
        let mut safety = 0;

        /* iterate capability linked list */
        while pos != 0 && safety < 64 {
            /* sanity checks: typical caps start at >= 0x40, pos must be in cfg-space */
            if pos == 0 {
                break;
            }
            let cap_id = unsafe { cfg.add(pos as usize).read_volatile() };
            let next = unsafe { cfg.add((pos + 1) as usize).read_volatile() };
            if cap_id == capability {
                unsafe { (pci_base.add(4) as *mut u16).write_volatile(old_cmd) };
                return Option::Some(pos);
            }
            pos = next;
            safety += 1;
        }
        return Option::None;
    }

    pub fn is_msi_x_capable(&self, pci_ident: u64) -> bool {
        return match self.get_capability(pci_ident, 0x11) {
            Some(_) => true,
            None => false,
        };
    }

    pub fn is_msi_capable(&self, pci_ident: u64) -> bool {
        return match self.get_capability(pci_ident, 0x5) {
            Some(_) => true,
            None => false,
        };
    }

    /**
     * returns Option::None, when pci_ident is invalid.
     */
    pub fn get_bar(&self, pci_ident: u64, bar: PciBarIndex) -> Option<PciBar> {
        for i in 0..self.pci_devices.entries {
            if *self.pci_devices.ref_const(i).unwrap() == pci_ident {
                let offset = match bar {
                    PciBarIndex::Index0 => 0x10,
                    PciBarIndex::Index1 => 0x14,
                    PciBarIndex::Index2 => 0x18,
                    PciBarIndex::Index3 => 0x1C,
                    PciBarIndex::Index4 => 0x20,
                    PciBarIndex::Index5 => 0x24,
                };

                let address = self.pci_base(pci_ident).unwrap();
                let original_value = unsafe { (address.add(offset) as *const u32).read_volatile() };

                let bar_type;
                let flags: u8;
                let data: u64;
                let length;
                if original_value & 1 == 0 {
                    bar_type = BarType::Memory;
                    if (original_value >> 1) & 3 == PCI_MEMORY_BAR_32BITS {
                        flags = ((original_value >> 3) & 2) as u8;
                        data = ((original_value >> 4) << 4) as u64;
                        unsafe { (address.add(offset) as *mut u32).write_volatile(0xFFFFFFFF) };
                        let mut new_value =
                            unsafe { (address.add(offset) as *const u32).read_volatile() };
                        unsafe { (address.add(offset) as *mut u32).write_volatile(original_value) };
                        new_value &= !7;
                        new_value = !new_value;
                        new_value += 1;
                        length = new_value;
                    } else {
                        let next_value =
                            unsafe { (address.add(offset + 4) as *const u32).read_volatile() };
                        flags = ((original_value >> 3) & 2) as u8;
                        data = ((original_value >> 4) << 4) as u64 | ((next_value as u64) << 32);
                        unsafe { (address.add(offset) as *mut u32).write_volatile(0xFFFFFFFF) };
                        let mut new_value =
                            unsafe { (address.add(offset) as *const u32).read_volatile() };
                        unsafe { (address.add(offset) as *mut u32).write_volatile(original_value) };
                        new_value &= !7;
                        new_value = !new_value;
                        new_value += 1;
                        length = new_value;
                    }
                } else {
                    bar_type = BarType::Io;
                    length = 0;
                    flags = 0;
                    data = ((original_value >> 2) << 2) as u64;
                }

                return Option::Some(PciBar {
                    bar_type,
                    flags,
                    data,
                    length,
                });
            }
        }
        return Option::None;
    }

    pub fn get_pin(&self, pci_device: u64) -> u8 {
        let base = match self.pci_base(pci_device) {
            Some(base) => base,
            None => simple_kernel_panic("PciBus/enable_bus_master", "Unknown device\n"),
        };

        return unsafe { *(base.add(0x3D) as *const u8) };
    }

    pub fn enable_bus_master(&self, pci_device: u64) {
        let base = match self.pci_base(pci_device) {
            Some(base) => base,
            None => simple_kernel_panic("PciBus/enable_bus_master", "Unknown device\n"),
        };
        let command = unsafe { base.add(4) } as *mut u16;

        unsafe {
            *command = *command | 1 << 2 | 1 << 1;
        }
    }
    pub fn enable_interrupts(&self, pci_device: u64) {
        let base = match self.pci_base(pci_device) {
            Some(base) => base,
            None => simple_kernel_panic("PciBus/enable_interrupts", "Unknown device\n"),
        };
        let command = unsafe { base.add(4) } as *mut u16;
        unsafe {
            *command = *command & !(1 << 10);
        }
    }

    pub fn initialize(&mut self, system_table: &mut SystemTable) {
        let mut module: Module<'static> = Module::new("Pci Bus");
        info!(module, "began initialization\n");
        let mcfg = match system_table.get_acpi_table(crate::acpi_tables::AcpiTableId::MCFG) {
            Some(mcfg) => unsafe { (*mcfg).get_ptr() as *const MCFG },
            None => {
                simple_kernel_panic(module.name(), "'MCFG' Acpi Table is missing\n");
            }
        };
        let length = unsafe { (*mcfg).header.get_length() };
        let number_tables = (length as usize - size_of::<MCFG>()) / 16;
        let __entry: *const EnhancedConfigurationMechanism =
            unsafe { &raw const (*mcfg).entries as *const EnhancedConfigurationMechanism };
        match system_table.page_4k(page_align(__entry), page_align(__entry), PAGER_PRESENT) {
            Ok(_) => {}
            Err(_e) => {
                simple_kernel_panic(module.name(), "Could not page MCFG\n");
            }
        };
        if number_tables != 1 {
            simple_kernel_panic(
                module.name(),
                "Unimplemented: More than 1 Enhanced configuration spaces\n",
            );
        }

        //TODO: add printing devices!

        let mut devices: DynamicAllocator<u64> =
            DynamicAllocator::new(&mut system_table.virtual_allocator.allocator, 4);
        let entry = EnhancedConfigurationMechanism {
            ..Default::default()
        };
        unsafe {
            memcpy_qword(addr_of!(entry) as *mut c_void, __entry as *const c_void, 2);
        };
        self.__pci_base = entry.base_address;
        let start_bus = entry.start_pci_bus_number;
        for bus in start_bus..2 {
            for device in 0..255 {
                let dev_desc = bus as u64 * 256 + device as u64 * 8;
                let mut addr = entry.base_address + dev_desc * 0x1000;
                match system_table.page_4k(addr, addr, PAGER_PRESENT | PAGER_RW | PAGER_PCD) {
                    Ok(_) => {}
                    Err(_e) => {
                        simple_kernel_panic(
                            module.name(),
                            "Could not page enhanced configration space device\n",
                        );
                    }
                };
                let vendor_id = unsafe { *(addr as *const u16) };
                if vendor_id == 0xFFFF {
                    match system_table.unpage_4k(addr) {
                        Some(_e) => {
                            simple_kernel_panic(
                                module.name(),
                                "Could not unpage enhanced configuration space device\n",
                            );
                        }
                        None => {}
                    };
                    continue;
                }
                let header_type = unsafe { *((addr + 0xC + 2) as *const u8) };
                if (header_type >> 7) == 1 {
                    for func in 0..8u8 {
                        addr = entry.base_address + ((dev_desc + func as u64) * 0x1000) as u64;
                        match system_table.page_4k(addr, addr, PAGER_PRESENT | PAGER_RW | PAGER_PCD)
                        {
                            Ok(_) => {}
                            Err(_e) => simple_kernel_panic(
                                module.name(),
                                "Could not page enhanced configration space device\n",
                            ),
                        }
                        let vendor_id = unsafe { *(addr as *const u16) };
                        if vendor_id != 0xFFFF {
                            match system_table.page_4k(
                                addr,
                                addr,
                                PAGER_PRESENT | PAGER_PCD | PAGER_RW,
                            ) {
                                Ok(_) => {}
                                Err(_e) => simple_kernel_panic(
                                    module.name(),
                                    "Could not page enhanced configration space device\n",
                                ),
                            }
                            match devices.push_back(
                                &mut system_table.virtual_allocator.allocator,
                                PciBus::compress_ident(bus, device, func),
                            ) {
                                Some(_e) => {
                                    simple_kernel_panic(module.name(), "Could not list device\n");
                                }
                                None => {}
                            }
                        } else {
                            match system_table.unpage_4k(addr) {
                                Some(_e) => {
                                    simple_kernel_panic(
                                        module.name(),
                                        "Could not unpage enhanced configuration space device\n",
                                    );
                                }
                                None => {}
                            };
                        }
                    }
                } else {
                    //page_kernel(addr, addr, PageCacheDisabled|PageWrite);
                    match devices.push_back(
                        &mut system_table.virtual_allocator.allocator,
                        PciBus::compress_ident(bus, device, 0),
                    ) {
                        Some(_e) => {
                            simple_kernel_panic(module.name(), "Could not list device\n");
                        }
                        None => {}
                    }
                }
            }
        }
        self.pci_devices = devices.to_list();
        info!(
            module,
            "total detected devices = {}\n",
            self.pci_devices.size()
        );
        success!(module, "finished initialization\n");
    }

    pub fn write_configuration_space_u32(&self, pci_device: u64, offset: u16, value: u32) {
        let mut base = match self.pci_base(pci_device) {
            Some(base) => base,
            None => simple_kernel_panic(
                "PciBus/write_configuration_space_u32",
                "Invalid pci_device\n",
            ),
        };

        unsafe {
            base = base.add(offset as usize);
            (base as *mut u32).write_volatile(value);
        }
    }

    pub fn read_configuration_space_u32(&self, pci_device: u64, offset: u16) -> u32 {
        let mut base = match self.pci_base(pci_device) {
            Some(base) => base,
            None => simple_kernel_panic(
                "PciBus/write_configuration_space_u32",
                "Invalid pci_device\n",
            ),
        };
        unsafe { base = base.add(offset as usize) };
        return unsafe { (base as *mut u32).read_volatile() };
    }

    pub fn new(system_table: &mut SystemTable) -> Self {
        let mut ret = Self::default();
        ret.initialize(system_table);
        return ret;
    }
}
