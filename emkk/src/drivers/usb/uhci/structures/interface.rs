use crate::drivers::usb::{
    standard_requests::UsbInterfaceDescriptor, traits::UsbInterface,
    uhci::structures::endpoint::UhciGeneralEndpoint,
};

pub struct UhciInterface {
    interface_class: u8,
    interface_sub_class: u8,
    interface_protocol: u8,
    interface_number: u8,
    alternate_setting: u8,
    pub(in crate::drivers::usb::uhci) endpoints: &'static mut [UhciGeneralEndpoint],
}

impl UhciInterface {
    pub fn from_raw(
        descriptor: &UsbInterfaceDescriptor,
        endpoints: &'static mut [UhciGeneralEndpoint],
    ) -> Self {
        return Self {
            interface_number: descriptor.b_interface_number,
            alternate_setting: descriptor.b_alternate_setting,
            interface_class: descriptor.b_interface_class,
            interface_sub_class: descriptor.b_interface_sub_class,
            interface_protocol: descriptor.b_interface_protocol,
            endpoints,
        };
    }
}

impl UsbInterface for UhciInterface {
    fn endpoint_count(&self) -> u16 {
        return self.endpoints.len() as u16;
    }
    fn get_class(&self) -> u8 {
        return self.interface_class;
    }
    fn get_endpoint(&self, index: u16) -> Option<&dyn crate::drivers::usb::traits::UsbEndpoint> {
        if index as usize >= self.endpoints.len() {
            return Option::None;
        }
        return Option::Some(&self.endpoints[index as usize]);
    }
    fn get_mut_endpoint(
        &mut self,
        index: u16,
    ) -> Option<&mut dyn crate::drivers::usb::traits::UsbEndpoint> {
        if index as usize >= self.endpoints.len() {
            return Option::None;
        }
        return Option::Some(&mut self.endpoints[index as usize]);
    }
    fn get_protocol(&self) -> u8 {
        return self.interface_protocol;
    }
    fn get_sub_class(&self) -> u8 {
        return self.interface_sub_class;
    }
}
