use crate::{
    aml::AmlCode,
    arch::apic::{Apic, IO_APIC_ACTIVE_HIGH, IO_APIC_DESTINATION_PHYSICAL, IO_APIC_LEVEL},
    drivers::usb::{
        ehci::{EHCI_CONTROLLER, create_ehci},
        independent::{UsbControllerType, UsbHidDeviceType},
        ohci::create_ohci,
        standard_requests::UsbHID,
        traits::{UsbController, UsbDevice},
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
pub struct Usb {
    ehci_present: bool,
    ohci_present: bool,
}

impl Default for Usb {
    fn default() -> Self {
        return Usb {
            ehci_present: false,
            ohci_present: false,
        };
    }
}

pub const FIXED_USB_INTERRUPT_GSI: u32 = 0x13; // = 19

//INFO: EHCI is currently on GSI 0x15!

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

                let gsi = routing_table
                    .find_pci_int(aml_code, device, func, pci_bus.get_pin(pci_device))
                    .unwrap();
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
        let ehci_present = match pci_bus.find_pci_device(0x20, 0x3, 0xC) {
            Some(pci_device) => {
                let processor = ref_processor_mut();
                let isr_vector = processor.request_isr_vector();

                let (bus, device, func) = PciBus::decompress_ident(pci_device);
                info!(
                    &mut module,
                    "Found Ehci at Bus {} Device {} Function {}\n", bus, device, func
                );

                let gsi = routing_table
                    .find_pci_int(aml_code, device, func, pci_bus.get_pin(pci_device))
                    .unwrap();
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

        success!(module, "Initialized\n");

        return Usb {
            ehci_present,
            ohci_present,
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
            _ => return Option::None,
        }
    }

    /* Second value is the size of the report descriptor*/
    pub fn find_hid_device(
        &self,
        hid_device: UsbHidDeviceType,
    ) -> Option<(UsbControllerType, &mut dyn UsbDevice, &UsbHID)> {
        return match hid_device {
            UsbHidDeviceType::Keyboard => {
                if !self.ehci_present {
                    return None;
                }
                let ehci_controller = unsafe { (&raw mut EHCI_CONTROLLER).as_mut() }.unwrap();
                let const_ehci_controller =
                    unsafe { (&raw const EHCI_CONTROLLER).as_ref() }.unwrap();
                let num_devices = ehci_controller.number_of_active_devices();
                for i in 0..num_devices {
                    let device = const_ehci_controller.get_device(i).unwrap();
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
                        return Some((
                            UsbControllerType::EHC,
                            ehci_controller.get_mut_device(i).unwrap(),
                            config.get_hid_interface(0).unwrap(),
                        ));
                    }
                }
                None
            }
            UsbHidDeviceType::Mouse => {
                if !self.ehci_present {
                    return None;
                }
                let ehci_controller = unsafe { (&raw mut EHCI_CONTROLLER).as_mut() }.unwrap();
                let const_ehci_controller =
                    unsafe { (&raw const EHCI_CONTROLLER).as_ref() }.unwrap();
                let num_devices = ehci_controller.number_of_active_devices();
                for i in 0..num_devices {
                    let device = const_ehci_controller.get_device(i).unwrap();
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
                        return Some((
                            UsbControllerType::EHC,
                            ehci_controller.get_mut_device(i).unwrap(),
                            config.get_hid_interface(0).unwrap(),
                        ));
                    }
                }
                None
            }
        };
    }
}
