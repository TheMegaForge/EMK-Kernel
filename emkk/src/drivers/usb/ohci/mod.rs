use core::{
    arch::asm,
    ffi::c_void,
    ptr::{null, null_mut}, slice,
};

use crate::{
    arch::{isr::ISRRegisters, lapic::LocalApic},
    drivers::usb::{
        independent::UsbControllerType,
        ohci::{data_structures::{OhciBar, OhciCommandStatusBitPart, OhciHcca, OhciInterrupt}, structures::{device::OhciDevice, interrupt_list::OhciHccaInterruptList, non_periodic_list::OhciNonPeriodicList}},
        traits::UsbController,
    },
    fixed_vaddrs::{OHCI_BAR_FIXED_VADDR, ref_processor_mut},
    hal::{
        memory::{
            allocator::{Allocator, MemoryBlock},
            pager::{PAGER_PCD, PAGER_PRESENT, PAGER_RW},
        },
        pci_bus::{PciBarIndex, PciBus},
        print::{Module, simple_kernel_panic},
    },
    info,
    time::sleep, utils::invalid_mut_slice,
};

pub mod data_structures;
pub mod structures;
pub struct OhciController {
    bar: OhciBar,
    hcca: OhciHcca,
    interrupt_list: OhciHccaInterruptList,
    bulk_list: OhciNonPeriodicList,
    control_list: OhciNonPeriodicList,
    present: bool,
    private_physical_allocator: Allocator,
    num_potential_devices: u8,
    num_active_devices: u8,
    error_present: bool,
    /**
     * Size = num_potential_devices
     * But only num_active_devices are valid
     */
    devices: &'static mut [OhciDevice],
    device_memory: MemoryBlock
}

impl OhciController {
    pub const fn empty() -> Self {
        return Self {
            bar: OhciBar::empty(),
            present: false,
            private_physical_allocator: Allocator::empty(),
            hcca: OhciHcca::new(null_mut()),
            interrupt_list: OhciHccaInterruptList::new(null_mut()),
            num_potential_devices: 0,
            num_active_devices: 0,
            error_present: false,
            bulk_list:  OhciNonPeriodicList::new(null_mut(), null_mut()),
            control_list: OhciNonPeriodicList::new(null_mut(), null_mut()),
            devices: invalid_mut_slice(),
            device_memory: MemoryBlock::empty(),
        };
    }
}

static mut OHCI_CONTROLLER: OhciController = OhciController::empty();

fn ohci_interrupt(_: &ISRRegisters) {
    unsafe { asm!("cli;nop") }
    LocalApic::from_local_core().send_eoi();
}

impl UsbController for OhciController {
    fn identity(&self) -> UsbControllerType {
        return UsbControllerType::OHC;
    }
    fn initialize_controller(
        &mut self,
        pci_bus: &PciBus,
        allocator: &mut Allocator,
        isr_vector: u8,
        pci_device: u64,
    ) -> bool {
        let mut module = Module::new("Ohci");

        let bar0 = match pci_bus.get_bar(pci_device, PciBarIndex::Index0) {
            Some(bar) => bar,
            None => simple_kernel_panic(module.name(), "Could not get bar0\n"),
        };

        let mut pages = bar0.get_length() / 0x1000;
        if bar0.get_length() % 0x1000 != 0 {
            pages += 1;
        }

        match ref_processor_mut().ref_mut_pager().page_4_kb(
            OHCI_BAR_FIXED_VADDR,
            bar0.get_address(),
            PAGER_RW | PAGER_PRESENT | PAGER_PCD,
            allocator,
        ) {
            Ok(_) => {}
            Err(_e) => simple_kernel_panic(module.name(), "Could not page bar\n"),
        };

        ref_processor_mut().install_isr(ohci_interrupt, isr_vector);

        let bar = OhciBar::new(OHCI_BAR_FIXED_VADDR as *mut c_void);
        self.bar = bar;
        self.present = true;
        self.private_physical_allocator = allocator.subdivide(32); /* 131.072 bytes for the ohci driver*/
        self.initalize(&mut module);
        return true;
    }

    fn configure_devices(&mut self) -> bool {
        todo!()
    }
    fn error_present(&self) -> bool {
        return self.error_present;
    }
    fn gather_device_information(&mut self) -> bool {
        todo!()
    }
    fn get_device(&self, index: u16) -> Option<&dyn super::traits::UsbDevice> {
        if index >= self.num_active_devices as u16 {
            return Option::None;
        }
        return Option::Some(&self.devices[index as usize]);
    }
    fn get_mut_device(&mut self, index: u16) -> Option<&mut dyn super::traits::UsbDevice> {
        if index >= self.num_active_devices as u16 {
            return Option::None;
        }
        return Option::Some(&mut self.devices[index as usize]);
    }
    fn install_interrupt_poller(
        &mut self,
        device: &dyn super::traits::UsbDevice,
        endpoint: &dyn super::traits::UsbEndpoint,
        frame: u8,
        report_address: u32,
        bytes_to_transfer: u16,
        callback: Option<super::traits::UsbInterruptPollerCallbackFn>,
    ) {
        todo!()
    }
    fn number_of_active_devices(&self) -> u16 {
        return self.num_active_devices as u16;
    }
    fn number_of_potential_devices(&self) -> u16 {
        return self.num_potential_devices as u16;
    }
    fn start(&mut self) {
        /*
         * NOTICE: Doesn´t deactivate the Controller.
         */
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Mie);
        self.bar.hc_control().enable_all_processing();
    }
    fn stop(&mut self) {
        /*
         * NOTICE: Doesn´t deactivate the Controller.
         */
        self.bar.hc_interrupt_disable().disable(OhciInterrupt::Mie);
        let mut hc_control = self.bar.hc_control().disable_all_processing();
    }
    fn untraited_work0(&mut self) -> Option<bool> {
        todo!()
    }
    fn untraited_work1(&mut self) -> Option<bool> {
        todo!()
    }
    fn untraited_work2(&mut self) -> Option<bool> {
        todo!()
    }
}
/*
 * In the far Future: Look at Section 5.2.10 and skip a pipe, if the bus is overused
 */
impl OhciController {
    /**
     * No SMM or BIOS driver, since it´s deactivated by ExitBootServices of the Bootloader
     * Notice: Full Speed MaxPacketSize = 8,16,32 or 64 bytes
     *         Low Speed MaxPacketSize = 8
     */
    pub fn initalize(&mut self, module: &mut Module<'static>) {
        info!(module, "Revision 0x{:x}\n", self.bar.hc_revision());
        let new_hcca_addr = match self.private_physical_allocator.alloc_zero(1) {
            Ok(mb) => mb.base,
            Err(_e) => simple_kernel_panic(module.name(), "Could not allocate memory for Hcca\n"),
        };
        self.reset();
        self.bar.write_hc_hcca(0xFFFFFFFF);
        if (!self.bar.hc_hcca()) + 1 > 0x1000 {
            simple_kernel_panic(module.name(), "Hcca alignment is greather than 0x1000\n")
        }
        self.bar.write_hc_hcca(new_hcca_addr as u32);
        self.hcca = OhciHcca::new(new_hcca_addr as *mut c_void);
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Mie);
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Oc);
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Ue);
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Rd);
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Wdh);
        // Total Timespan from SOF to SOF is 1ms
        self.bar.write_hc_periodic_start(0x2A2F); // Interrupt/Isochronous List is preferred after ~900 microseconds
        self.bar.write_hc_ls_threshold(0x500); // 1280 bit times. 1 bit time = 83,33ns (or 1 cycle from 12 Mhz clock)
        /*
         * FSMPS = floor( (FrameInterval - 210) * 6/7 )
         */
        self.bar
            .hc_fm_interval()
            .set_part(data_structures::OhciFmIntervalPart::Fsmps, 0x2778);

        self.interrupt_list = OhciHccaInterruptList::new(new_hcca_addr as *mut u32);
        self.interrupt_list.initialize(&mut self.private_physical_allocator);

        self.bulk_list = unsafe { OhciNonPeriodicList::new(self.bar.address().add(11), self.bar.address().add(10)) };
        self.control_list = unsafe { OhciNonPeriodicList::new(self.bar.address().add(9), self.bar.address().add(8)) };

        self.num_potential_devices =
            self.bar
                .hc_rh_descriptor_a()
                .get(data_structures::OhciRhDescriptorAPart::Ndp) as u8;
        info!(module, "Potential Devices {}\n", self.num_potential_devices);

        self.device_memory = match self.private_physical_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(module.name(), "Could not allocate Memory for Device Array\n")
        };
        self.devices = unsafe { slice::from_raw_parts_mut(self.device_memory.as_mut_ptr(), self.num_potential_devices as usize) };
    }
    /**
     * Causes
     *  1:1 Control/Bulk Servicing Ratio
     *  Isochronous, Periodic, Bulk, Control, Interrupt Lists to be disabled
     *  HCFS to be set to 'UsbReset'
     *  Control List Filled to be set to 0
     */
    pub fn reset(&mut self) {
        self.bar
            .hc_command_status()
            .set(OhciCommandStatusBitPart::Hcr);
        sleep(10);

        while self
            .bar
            .hc_command_status()
            .is_set(OhciCommandStatusBitPart::Hcr)
        { /* Wait*/ }
    }
}

pub fn create_ohci(
    pci_bus: &PciBus,
    pci_device: u64,
    physical_allocator: &mut Allocator,
    isr_vector: u8,
) {
    unsafe {
        let ohci_controller = &raw mut OHCI_CONTROLLER;
        (*ohci_controller).initialize_controller(
            pci_bus,
            physical_allocator,
            isr_vector,
            pci_device,
        );
        (*ohci_controller).untraited_work0();
        (*ohci_controller).gather_device_information();
        (*ohci_controller).configure_devices();
        (*ohci_controller).untraited_work2();
    }
}
