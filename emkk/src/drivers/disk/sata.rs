/*
 * Specifications used
 *      Serial Ata Ahci Revision 1-3-1
 */

use core::{ffi::c_void, ptr::null_mut};

use crate::{
    drivers::disk::{ControllerType, Disk, DiskController, DiskIOResult, sata_abar::Abar},
    hal::{
        memory::{
            allocator::{Allocator, MemoryBlock},
            pager::Pager,
        },
        pci_bus::{PciBarIndex, PciBus},
        print::{Module, simple_kernel_panic},
    },
    info, success,
    time::sleep,
    utils::{buffer::Buffer, traits::Region},
};

pub struct SataDisk {
    identifier_: u32,
    num_sectors_: u32,
    sector_size_: u32,
}

impl Disk for SataDisk {
    fn read_into_buffer(&self, lba: u64, buffer: &Buffer, scheduler: u8) -> DiskIOResult {
        DiskIOResult::InvalidLba
    }
    fn write_from_buffer(&self, lba: u64, buffer: &Buffer, scheduler: u8) -> DiskIOResult {
        DiskIOResult::InvalidLba
    }

    fn identifier(&self) -> u32 {
        return self.identifier_;
    }
    fn num_sectors(&self) -> u64 {
        return self.num_sectors_ as u64;
    }
    fn sector_size(&self) -> u32 {
        return self.sector_size_;
    }
}

pub struct SataController {
    pci_device: u64,
    disks: *mut SataDisk,
    disks_connected: u8,
    abar: Abar,
    present: bool,
}
static mut TEMP_PORT_SPEED: [u8; 32] = [0; 32];
impl SataController {
    fn reset_controller(module: &mut Module<'static>, bar: &Abar) {
        if !bar.cap().supports_64bit_addressing() {
            simple_kernel_panic(module.name(), "Sata Controll is not 64 bit addressable\n");
        }
        if bar.cap2().bios_os_handoff() {
            let mut bohc = bar.bohc();
            bohc.set_os_owned_semaphore(true);

            while bohc.bios_owned_semaphore() {}
            while bohc.bios_busy() {}
            sleep(200);
            /* OS has control of Sata Controller*/
        }
        let ports_implemented = bar.pi();
        if bar.cap().supports_staggered_spin_up() {
            for i in 0..31 {
                if ports_implemented & (1 << i) != 0 {
                    let mut port = bar.port(i);
                    port.cmd().set_start(false);

                    let mut count = 50;
                    while count != 0 {
                        sleep(10);
                        if !port.cmd().command_list_running() {
                            break;
                        }
                        count -= 1
                    }
                    if count == 0 {
                        simple_kernel_panic(module.name(), "Could not reset port; out of time\n");
                    }
                    port.sctl().set_device_detection_initialization(1);
                    sleep(20);
                    port.sctl().set_device_detection_initialization(0);
                    while port.ssts().device_detection() == 3 {}
                    port.serr().clear_all();
                    /* Chapter 10.4.2*/
                }
            }
            /* Ports are reset; Chapter 3.1.2 Bit 0 */
        }

        for i in 0..31 {
            if ports_implemented & (1 << i) != 0 {
                let port = bar.port(i);
                unsafe { TEMP_PORT_SPEED[i as usize] = port.ssts().current_interface_speed() }
            }
        }

        bar.ghc().set_hba_reset(true);

        let mut count = 100;
        while count != 0 {
            sleep(10);
            if !bar.ghc().hba_reset() {
                break;
            }
            count -= 1;
        }
        if count == 0 {
            simple_kernel_panic(module.name(), "Could not reset HBA\n");
        }
        bar.ghc().set_ahci_enable(true);
        sleep(200);
        /* Control is reset and ports should report signatures!\n*/
        info!(module, "controller has been reset\n");
    }
    /*
     * TODO:
     * Sets speed
     * Creates and allocates structures
     */
    fn initialize_ports(&mut self, module: &mut Module<'static>, allocator: &mut Allocator) {
        let ports_implemented = self.abar.pi();
        for i in 0..31 {
            if ports_implemented & (1 << i) != 0 {
                let mut port = self.abar.port(i);
                let command_list_base = match allocator.alloc_zero(1) {
                    Ok(mb) => mb.base,
                    Err(_e) => {
                        simple_kernel_panic(module.name(), "Could not allocate command list\n")
                    }
                };
                let fis_base_address = match allocator.alloc_zero(1) {
                    Ok(mb) => mb.base,
                    Err(_e) => {
                        simple_kernel_panic(module.name(), "Could not allocate recieved FIS list\n")
                    }
                };
                port.set_clb((command_list_base & 0xFFFFFFFF) as u32);
                port.set_clbu((command_list_base >> 32) as u32);
                port.set_fb((fis_base_address & 0xFFFFFFFF) as u32);
                port.set_fbu((fis_base_address >> 32) as u32);

                port.sctl()
                    .set_speed_allowed(unsafe { TEMP_PORT_SPEED[i as usize] });
                port.cmd().set_fis_receive_enable(true);

                let sig = port.signature();
                if sig != 0x96690101 && sig != 0x00000101 {
                    match allocator.free(&MemoryBlock::new(0x1000, fis_base_address)) {
                        Ok(_) => {}
                        Err(_e) => simple_kernel_panic(module.name(), "Could not free fis base\n"),
                    }
                    match allocator.free(&MemoryBlock::new(0x1000, command_list_base)) {
                        Ok(_) => {}
                        Err(_e) => {
                            simple_kernel_panic(module.name(), "Could not free command list\n")
                        }
                    }
                    continue;
                } else if sig == 0x96690101 {
                    if self.abar.cap().supports_port_multiplier() {
                        port.cmd().set_port_multiplier_attached(true);
                    }
                }
                port.cmd().set_start(true);
            }
        }
        info!(module, "Ports have been initialized\n");
    }
}

impl SataController {
    pub fn not_present() -> Self {
        return SataController {
            pci_device: 0,
            disks: null_mut(),
            disks_connected: 0,
            abar: Abar::empty(),
            present: false,
        };
    }
}

impl DiskController for SataController {
    fn new(
        pci_bus: &PciBus,
        pci_device: u64,
        allocator: &mut Allocator,
        pager: &mut Pager,
        isr_vector: u8,
        dst: &mut SataController,
    ) {
        let mut module = Module::new("Sata");
        let _disk_connected = 0;

        pci_bus.enable_bus_master(pci_device);
        pci_bus.enable_interrupts(pci_device);
        let raw_bar = pci_bus.get_bar(pci_device, PciBarIndex::Index5).unwrap();
        raw_bar.map(pager, allocator);
        let abar = Abar::new(raw_bar.get_address() as *mut c_void);

        SataController::reset_controller(&mut module, &abar);

        dst.initialize_ports(&mut module, allocator);
        success!(&mut module, "Initialized\n");
    }
    fn present(&self) -> bool {
        return self.present;
    }
    fn identify(&self) -> ControllerType {
        return ControllerType::Sata;
    }
    fn num_disks_present(&self) -> u8 {
        return self.disks_connected;
    }
    fn hotplugging_is_supported(&self) -> bool {
        return true;
    }

    fn get_disk_mut(&mut self, disk: u32) -> Option<&'static mut dyn Disk> {
        for i in 0..self.disks_connected {
            let sata_disk = unsafe { self.disks.add(i as usize).as_ref().unwrap() };
            if sata_disk.identifier() == disk {
                return Option::Some(unsafe { self.disks.add(i as usize).as_mut().unwrap() });
            }
        }
        return Option::None;
    }
    fn get_disk_indexed_mut(&mut self, index: u8) -> Option<(u32, &'static mut dyn Disk)> {
        if index < self.disks_connected {
            let sata_disk = unsafe { self.disks.add(index as usize).as_mut().unwrap() };
            return Option::Some((sata_disk.identifier_, sata_disk));
        }
        return Option::None;
    }
    fn get_disk(&self, identifier: u32) -> Option<&'static dyn Disk> {
        for i in 0..self.disks_connected {
            let sata_disk = unsafe { self.disks.add(i as usize).as_ref().unwrap() };
            if sata_disk.identifier() == identifier {
                return Option::Some(sata_disk);
            }
        }
        return Option::None;
    }
    fn get_disk_indexed(&self, index: u8) -> Option<(u32, &'static dyn Disk)> {
        if index < self.disks_connected {
            let sata_disk = unsafe { self.disks.add(index as usize).as_ref().unwrap() };
            return Option::Some((sata_disk.identifier_, sata_disk));
        }
        return Option::None;
    }
}
