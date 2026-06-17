use core::ptr::null_mut;

use crate::{
    drivers::usb::{
        ehci::{
            EHCI_CONTROLLER, configuration_parser::EhciDeviceConfiguration,
            data_structures::QueueHead, structures::endpoint::EhciEndpoint,
        },
        independent::{UsbDeviceInformation, UsbDeviceState},
        standard_requests::UsbStandardDeviceRequest,
        traits::{UsbConfiguration, UsbDevice},
    },
    hal::{memory::allocator::Allocator, print::simple_kernel_panic},
};

pub struct EhciDevice {
    pub(in crate::drivers::usb::ehci) port_num: u8,
    pub(in crate::drivers::usb::ehci) num_endpoints: u8,
    pub(in crate::drivers::usb::ehci) address: u8,
    pub(in crate::drivers::usb::ehci) state: UsbDeviceState,
    pub(in crate::drivers::usb::ehci) default_control_endpoint: EhciEndpoint,
    pub(in crate::drivers::usb::ehci) default_control_endpoint_setup_data_packet_base: u32,
    pub(in crate::drivers::usb::ehci) device_information: UsbDeviceInformation,
    pub(in crate::drivers::usb::ehci) configurations: *const EhciDeviceConfiguration,
    pub(in crate::drivers::usb::ehci) num_configurations: u8,
}

impl EhciDevice {
    pub fn new_reset(
        allocator: &mut Allocator,
        port_num: u8,
        address: u8,
        default_control_endpoint_setup_data_packet_base: u32,
    ) -> Self {
        let endpoint_qtd_memory = match allocator.alloc_zero(1) {
            Ok(mb) => mb.as_mut_ptr(),
            Err(_e) => simple_kernel_panic(
                "Ehci/reset_port",
                "COuld not allocate default control endpoint\n",
            ),
        };
        return Self {
            port_num,
            num_endpoints: 1,
            address,
            state: UsbDeviceState::Resetted,
            default_control_endpoint: EhciEndpoint::new(
                0,
                endpoint_qtd_memory,
                128,
                QueueHead::new(0),
            ),
            default_control_endpoint_setup_data_packet_base,
            device_information: UsbDeviceInformation::empty(),
            configurations: null_mut(),
            num_configurations: 0,
        };
    }
}

impl Default for EhciDevice {
    fn default() -> Self {
        return Self {
            port_num: 0,
            address: 0,
            state: UsbDeviceState::Invalid,
            num_endpoints: 1,
            default_control_endpoint: EhciEndpoint::default(),
            default_control_endpoint_setup_data_packet_base: 0,
            device_information: UsbDeviceInformation::empty(),
            configurations: null_mut(),
            num_configurations: 0,
        };
    }
}

impl UsbDevice for EhciDevice {
    fn await_interrupt(&self) {
        while unsafe { !EHCI_CONTROLLER.endpoint_interrupted } {}
        unsafe { EHCI_CONTROLLER.endpoint_interrupted = false };
    }

    fn detach(&mut self) {
        self.state = UsbDeviceState::Detached;
        self.port_num = 0xFF;
    }

    fn get_class_code(&self) -> u8 {
        return self.device_information.device_class;
    }
    fn get_sub_class_code(&self) -> u8 {
        return self.device_information.device_sub_class;
    }

    fn get_port(&self) -> u8 {
        return self.port_num;
    }
    fn get_state(&self) -> UsbDeviceState {
        return self.state;
    }
    fn endpoint_count(&self) -> u8 {
        return self.num_endpoints;
    }
    fn device_address(&self) -> u8 {
        return self.address;
    }
    fn request_packet_address<'a>(&'a self) -> &'a mut UsbStandardDeviceRequest {
        return unsafe {
            (self.default_control_endpoint_setup_data_packet_base as *mut UsbStandardDeviceRequest)
                .as_mut()
                .unwrap()
        };
    }
    fn get_configuration_count(&self) -> u8 {
        return self.num_configurations;
    }
    fn get_configuration(&self, config: u8) -> Option<&dyn UsbConfiguration> {
        if config > self.num_configurations {
            return Option::None;
        }
        unsafe {
            let ptr = self.configurations.add(config as usize);
            let ret = &*ptr;
            return Option::Some(ret);
        }
    }
}
