use crate::drivers::usb::{
    independent::{UsbDeviceInformation, UsbDeviceState},
    traits::UsbDevice,
    uhci::{
        configuration_parser::UhciDeviceConfiguration,
        structures::{UHCI_DUMMY_CONTROL_ENDPOINT, endpoint::UhciControlEndpoint},
    },
};

pub struct UhciDevice {
    pub(in crate::drivers::usb::uhci) device_address: u8,
    class_code: u8,
    sub_class_code: u8,
    port: u8,
    pub(in crate::drivers::usb::uhci) state: UsbDeviceState,
    pub(in crate::drivers::usb::uhci) control_endpoint: &'static mut UhciControlEndpoint,
    signaled: bool,
    pub(in crate::drivers::usb::uhci) device_information: UsbDeviceInformation,
    pub(in crate::drivers::usb::uhci) configuration: UhciDeviceConfiguration,
}

impl UhciDevice {
    pub fn new_detached() -> Self {
        return Self {
            device_address: 0,
            class_code: 0,
            sub_class_code: 0,
            port: 0,
            state: UsbDeviceState::Detached,
            control_endpoint: unsafe { &mut *(&raw mut UHCI_DUMMY_CONTROL_ENDPOINT) },
            signaled: false,
            device_information: UsbDeviceInformation::empty(),
            configuration: UhciDeviceConfiguration::empty(),
        };
    }

    pub fn new_addressed(port: u8) -> Self {
        return Self {
            device_address: 0,
            class_code: 0,
            sub_class_code: 0,
            port,
            state: UsbDeviceState::Address,
            control_endpoint: unsafe { &mut *(&raw mut UHCI_DUMMY_CONTROL_ENDPOINT) },
            signaled: false,
            device_information: UsbDeviceInformation::empty(),
            configuration: UhciDeviceConfiguration::empty(),
        };
    }

    pub fn attach_control_endpoint(&mut self, control_endpoint: &'static mut UhciControlEndpoint) {
        self.control_endpoint = control_endpoint;
    }

    pub fn await_interrupt(&mut self) {
        while !self.signaled {}
        self.signaled = false;
    }

    pub fn signal(&mut self) {
        self.signaled = true;
    }
}

impl UsbDevice for UhciDevice {
    fn detach(&mut self) {
        todo!()
    }
    fn device_address(&self) -> u8 {
        return self.device_address;
    }
    fn get_class_code(&self) -> u8 {
        return self.class_code;
    }
    fn get_sub_class_code(&self) -> u8 {
        return self.sub_class_code;
    }
    fn get_configuration(
        &self,
        config: u8,
    ) -> Option<&dyn crate::drivers::usb::traits::UsbConfiguration> {
        if config != 0 {
            return Option::None;
        }
        return Option::Some(&self.configuration);
    }
    fn get_mut_configuration(
        &mut self,
        config: u8,
    ) -> Option<&mut dyn crate::drivers::usb::traits::UsbConfiguration> {
        if config != 0 {
            return Option::None;
        }
        return Option::Some(&mut self.configuration);
    }
    fn get_configuration_count(&self) -> u8 {
        return 1;
    }
    fn get_port(&self) -> u8 {
        return self.port;
    }
    fn get_state(&self) -> UsbDeviceState {
        return self.state;
    }
}
