use crate::drivers::usb::{
    independent::{UsbDeviceInformation, UsbDeviceState},
    traits::UsbDevice,
    xhci::{
        configuration_parser::XhciDeviceConfiguration,
        structures::{contexts::XhciInputContext32, endpoint::XhciEndpoint},
    },
};

pub struct XhciDevice {
    device_context: u64,
    device_address: u8,
    pub(in crate::drivers::usb::xhci) slot_id: u16,
    port_index: u8,
    pub(in crate::drivers::usb::xhci) input_context: &'static mut XhciInputContext32,
    pub(in crate::drivers::usb::xhci) control_endpoint: &'static mut XhciEndpoint,
    state: UsbDeviceState,
    pub(in crate::drivers::usb::xhci) device_information: UsbDeviceInformation,
    pub(in crate::drivers::usb::xhci) signaled: bool,
    pub(in crate::drivers::usb::xhci) configuration: XhciDeviceConfiguration,
}

impl XhciDevice {
    pub fn new(
        slot_id: u16,
        port_index: u8,
        device_context_addr: u64,
        input_context: &'static mut XhciInputContext32,
        control_endpoint: &'static mut XhciEndpoint,
    ) -> Self {
        return Self {
            slot_id,
            port_index,
            device_context: device_context_addr,
            input_context,
            device_address: 0,
            control_endpoint,
            state: UsbDeviceState::Detached,
            device_information: UsbDeviceInformation::empty(),
            configuration: XhciDeviceConfiguration::empty(),
            signaled: false,
        };
    }
    #[inline(always)]
    pub fn set_address(&mut self) {
        self.device_address = unsafe { *((self.device_context + 0xC) as *const u8) };
        self.state = UsbDeviceState::Address;
    }

    pub fn await_interrupt(&mut self) {
        while !self.signaled {}
        self.signaled = false;
    }
    #[inline(always)]
    pub fn get_device_context(&self, dci: u64) -> u64 {
        self.device_context + 0x20 * dci
    }
}

impl UsbDevice for XhciDevice {
    fn detach(&mut self) {
        todo!()
    }
    fn device_address(&self) -> u8 {
        return self.device_address;
    }
    fn get_class_code(&self) -> u8 {
        return self.device_information.device_class;
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
    fn get_configuration_count(&self) -> u8 {
        return 1;
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
    fn get_port(&self) -> u8 {
        return self.port_index;
    }
    fn get_state(&self) -> crate::drivers::usb::independent::UsbDeviceState {
        return self.state;
    }
    fn get_sub_class_code(&self) -> u8 {
        return self.device_information.device_sub_class;
    }
}
