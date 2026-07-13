use crate::{
    aml::AmlCode,
    arch::apic::{Apic, IO_APIC_ACTIVE_HIGH, IO_APIC_DESTINATION_PHYSICAL, IO_APIC_LEVEL},
    drivers::usb::{
        ehci::{EHCI_CONTROLLER, create_ehci},
        independent::{
            UsbControllerType::{self, OHC},
            UsbHidDeviceType,
        },
        ohci::{OHCI_CONTROLLER, create_ohci},
        standard_requests::UsbHID,
        traits::{UsbController, UsbDevice},
        uhci::{UHCI_CONTROLLER, create_uhci},
        xhci::{XHCI_CONTROLLER, create_xhci},
    },
    fixed_vaddrs::ref_processor_mut,
    hal::{PciRoutingTable, memory::allocator::Allocator, pci_bus::PciBus, print::Module},
    info,
    multithreading::processors::Processor,
    success,
};

pub mod ehci;
pub mod independent;
pub mod ohci;
pub mod standard_requests;
pub mod traits;
pub mod uhci;
pub mod xhci;
pub struct Usb {
    ehci_present: bool,
    ohci_present: bool,
    uhci_present: bool,
    xhci_present: bool,
}

impl Default for Usb {
    fn default() -> Self {
        return Usb {
            ehci_present: false,
            ohci_present: false,
            uhci_present: false,
            xhci_present: false,
        };
    }
}

impl Usb {
    pub fn new(
        pci_bus: &PciBus,
        physical_allocator: &mut Allocator,
        apic: &mut Apic,
        routing_table: &PciRoutingTable,
        aml_code: &mut AmlCode,
    ) -> Self {
        let mut module = Module::new("Usb");
        let ohci_present = match pci_bus.find_pci_device(0x10, 0x3, 0xC) {
            Some(pci_device) => {
                let processor = ref_processor_mut();
                let isr_vector = processor.request_isr_vector();

                let (bus, device, func) = PciBus::decompress_ident(pci_device);
                info!(
                    &mut module,
                    "Found Ohci at Bus {} Device {} Function {}\n", bus, device, func
                );

                let mut pin = pci_bus.get_pin(pci_device);
                assert_ne!(pin, 0);
                pin -= 1;

                let gsi = routing_table
                    .find_pci_int(aml_code, device, func, pin)
                    .unwrap();
                info!(&mut module, "Ohci uses apic-irq {}\n", gsi.irq);
                apic.write_gsi(
                    gsi.irq as u32,
                    isr_vector,
                    IO_APIC_DESTINATION_PHYSICAL,
                    gsi.polarity,
                    (!gsi.polarity) & 1,
                    processor.get_lapic().get_id() as u32,
                );
                pci_bus.enable_bus_master(pci_device);
                pci_bus.enable_interrupts(pci_device);
                create_ohci(pci_bus, pci_device, physical_allocator, isr_vector);
                true
            }
            None => false,
        };
        let uhci_present = match pci_bus.find_pci_device(0x0, 0x3, 0xC) {
            Some(pci_device) => {
                let processor = ref_processor_mut();
                let isr_vector = processor.request_isr_vector();

                let (bus, device, func) = PciBus::decompress_ident(pci_device);
                info!(
                    &mut module,
                    "Found Uhci at Bus {} Device {} Function {}\n", bus, device, func
                );

                let mut pin = pci_bus.get_pin(pci_device);
                assert_ne!(pin, 0);
                pin -= 1;

                let gsi = routing_table
                    .find_pci_int(aml_code, device, func, pin)
                    .unwrap();
                info!(&mut module, "Uhci uses apic-irq {}\n", gsi.irq);
                apic.write_gsi(
                    gsi.irq as u32,
                    isr_vector,
                    IO_APIC_DESTINATION_PHYSICAL,
                    gsi.polarity,
                    (!gsi.trigger_mode) & 1, // this is needed
                    processor.get_lapic().get_id() as u32,
                );
                pci_bus.enable_bus_master(pci_device);
                pci_bus.enable_interrupts(pci_device);
                pci_bus.enable_io_space(pci_device);
                create_uhci(pci_bus, pci_device, physical_allocator, isr_vector);
                true
            }
            None => false,
        };
        let ehci_present = match pci_bus.find_pci_device(0x20, 0x3, 0xC) {
            Some(pci_device) => {
                let processor = ref_processor_mut();
                let isr_vector = processor.request_isr_vector();

                let (bus, device, func) = PciBus::decompress_ident(pci_device);
                info!(
                    &mut module,
                    "Found Ehci at Bus {} Device {} Function {}\n", bus, device, func
                );

                let mut pin = pci_bus.get_pin(pci_device);
                assert_ne!(pin, 0);
                pin -= 1;

                let gsi = routing_table
                    .find_pci_int(aml_code, device, func, pin)
                    .unwrap();
                info!(&mut module, "Ehci uses apic-irq {}\n", gsi.irq);
                apic.write_gsi(
                    gsi.irq as u32,
                    isr_vector,
                    IO_APIC_DESTINATION_PHYSICAL,
                    gsi.polarity,
                    (!gsi.trigger_mode) & 1, // this is needed
                    processor.get_lapic().get_id() as u32,
                );
                pci_bus.enable_bus_master(pci_device);
                pci_bus.enable_interrupts(pci_device);
                create_ehci(pci_bus, pci_device, physical_allocator, isr_vector);
                true
            }
            None => false,
        };
        let xhci_present = match pci_bus.find_pci_device(0x30, 0x3, 0xC) {
            Some(pci_device) => {
                let processor = ref_processor_mut();
                let isr_vector = processor.request_isr_vector();

                let (bus, device, func) = PciBus::decompress_ident(pci_device);
                info!(
                    &mut module,
                    "Found Xhci at Bus {} Device {} Function {}\n", bus, device, func
                );

                let mut pin = pci_bus.get_pin(pci_device);
                assert_ne!(pin, 0);
                pin -= 1;

                let gsi = routing_table
                    .find_pci_int(aml_code, device, func, pin)
                    .unwrap();
                info!(&mut module, "Xhci uses apic-irq {}\n", gsi.irq);
                apic.write_gsi(
                    gsi.irq as u32,
                    isr_vector,
                    IO_APIC_DESTINATION_PHYSICAL,
                    gsi.polarity,
                    (!gsi.trigger_mode) & 1, // this is needed
                    processor.get_lapic().get_id() as u32,
                );
                pci_bus.enable_bus_master(pci_device);
                pci_bus.enable_interrupts(pci_device);
                create_xhci(pci_bus, pci_device, physical_allocator, isr_vector);
                true
            }
            None => false,
        };

        success!(module, "Initialized\n");
        return Usb {
            xhci_present,
            ehci_present,
            ohci_present,
            uhci_present,
        };
    }

    pub fn get_mut_controller(
        &self,
        controller_type: UsbControllerType,
    ) -> Option<&mut dyn UsbController> {
        match controller_type {
            UsbControllerType::EHC => {
                if !self.ehci_present {
                    return Option::None;
                }
                let ret = unsafe { &mut *(&raw mut EHCI_CONTROLLER) };
                return Option::Some(ret);
            }
            UsbControllerType::OHC => {
                if !self.ohci_present {
                    return Option::None;
                }
                let ret = unsafe { &mut *(&raw mut OHCI_CONTROLLER) };
                return Option::Some(ret);
            }
            UsbControllerType::UHC => {
                if !self.uhci_present {
                    return Option::None;
                }
                let ret = unsafe { &mut *(&raw mut UHCI_CONTROLLER) };
                return Option::Some(ret);
            }
            UsbControllerType::XHC => {
                if !self.xhci_present {
                    return Option::None;
                }
                let ret = unsafe { &mut *(&raw mut XHCI_CONTROLLER) };
                return Option::Some(ret);
            }
        }
    }

    /* This is just terrible, rewrite it later.*/
    /* Second value is the size of the report descriptor*/
    pub fn find_hid_device(
        &self,
        hid_device: UsbHidDeviceType,
    ) -> Option<(UsbControllerType, &mut dyn UsbDevice, &UsbHID)> {
        let present: [bool; 4] = [
            self.xhci_present,
            self.ehci_present,
            self.ohci_present,
            self.uhci_present,
        ];
        #[allow(static_mut_refs)]
        let mut_controllers: [&'static mut dyn UsbController; 4] = unsafe {
            [
                &mut XHCI_CONTROLLER,
                &mut EHCI_CONTROLLER,
                &mut OHCI_CONTROLLER,
                &mut UHCI_CONTROLLER,
            ]
        };
        #[allow(static_mut_refs)]
        let const_controllers: [&'static dyn UsbController; 4] = unsafe {
            [
                &XHCI_CONTROLLER,
                &EHCI_CONTROLLER,
                &OHCI_CONTROLLER,
                &UHCI_CONTROLLER,
            ]
        };
        let controller_types: [UsbControllerType; 4] = [
            UsbControllerType::XHC,
            UsbControllerType::EHC,
            UsbControllerType::OHC,
            UsbControllerType::UHC,
        ];
        return match hid_device {
            UsbHidDeviceType::Keyboard => {
                for i in 0..4 {
                    if !present[i] {
                        continue;
                    }
                    let mut_controller = unsafe {
                        &mut *(mut_controllers[i as usize] as *mut (dyn UsbController + 'static))
                    };
                    let const_controller = const_controllers[i as usize];
                    let num_devices = const_controller.number_of_active_devices();
                    for dev_index in 0..num_devices {
                        let device = const_controller.get_device(dev_index).unwrap();
                        if device.get_class_code() != 0 || device.get_sub_class_code() != 0 {
                            continue;
                        }
                        let config = device.get_configuration(0).unwrap();
                        let interface = config.get_interface(0).unwrap();
                        if interface.get_class() != 3 {
                            continue;
                        }
                        /* 1 = KBD*/
                        if interface.get_protocol() == 1 {
                            let r#type = controller_types[i as usize];
                            return Some((
                                r#type,
                                mut_controller.get_mut_device(dev_index as u16).unwrap(),
                                config.get_hid_interface(0).unwrap(),
                            ));
                        }
                    }
                }

                None
            }
            UsbHidDeviceType::Mouse => {
                for i in 0..4 {
                    if !present[i] {
                        continue;
                    }
                    let mut_controller = unsafe {
                        &mut *(mut_controllers[i as usize] as *mut (dyn UsbController + 'static))
                    };
                    let const_controller = const_controllers[i as usize];
                    let num_devices = const_controller.number_of_active_devices();
                    for dev_index in 0..num_devices {
                        let device = const_controller.get_device(dev_index).unwrap();
                        if device.get_class_code() != 0 || device.get_sub_class_code() != 0 {
                            continue;
                        }
                        let config = device.get_configuration(0).unwrap();
                        let interface = config.get_interface(0).unwrap();
                        if interface.get_class() != 3 {
                            continue;
                        }
                        /* 2 = MOU */
                        if interface.get_protocol() == 2 {
                            let r#type = controller_types[i as usize];
                            return Some((
                                r#type,
                                mut_controller.get_mut_device(dev_index as u16).unwrap(),
                                config.get_hid_interface(0).unwrap(),
                            ));
                        }
                    }
                }
                None
            }
        };
    }
}
