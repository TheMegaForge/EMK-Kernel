use crate::drivers::usb::{
    standard_requests::UsbInterfaceDescriptor,
    traits::UsbInterface,
    xhci::structures::endpoint::{XhciEndpoint, XhciEndpointDescriptor},
};

pub struct XhciInterface {
    class: u8,
    sub_class: u8,
    protocol: u8,
    pub(in crate::drivers::usb::xhci) endpoints: &'static mut [XhciEndpointDescriptor],
}

impl XhciInterface {
    pub fn from_raw(
        interface_descriptor: &UsbInterfaceDescriptor,
        endpoints: &'static mut [XhciEndpointDescriptor],
    ) -> Self {
        return Self {
            class: interface_descriptor.b_interface_class,
            sub_class: interface_descriptor.b_interface_sub_class,
            protocol: interface_descriptor.b_interface_protocol,
            endpoints,
        };
    }
}

impl UsbInterface for XhciInterface {
    fn endpoint_count(&self) -> u16 {
        return self.endpoints.len() as u16;
    }
    fn get_class(&self) -> u8 {
        return self.class;
    }
    fn get_endpoint(&self, index: u16) -> Option<&dyn crate::drivers::usb::traits::UsbEndpoint> {
        if index >= self.endpoint_count() {
            return Option::None;
        }
        return Option::Some(&self.endpoints[index as usize]);
    }
    fn get_mut_endpoint(
        &mut self,
        index: u16,
    ) -> Option<&mut dyn crate::drivers::usb::traits::UsbEndpoint> {
        if index >= self.endpoint_count() {
            return Option::None;
        }
        return Option::Some(&mut self.endpoints[index as usize]);
    }
    fn get_protocol(&self) -> u8 {
        return self.protocol;
    }
    fn get_sub_class(&self) -> u8 {
        return self.sub_class;
    }
}
