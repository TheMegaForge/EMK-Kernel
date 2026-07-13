use core::ptr::null_mut;

use crate::{
    drivers::usb::{
        ehci::structures::endpoint::EhciEndpoint,
        standard_requests::UsbInterfaceDescriptor,
        traits::{UsbEndpoint, UsbInterface},
    },
    utils::slices::invalid_mut_slice,
};

pub struct EhciInterface {
    num_endpoints: u8,
    interface_class: u8,
    interface_sub_class: u8,
    interface_protocol: u8,
    i_interface: u8,
    endpoints: &'static mut [EhciEndpoint],
}

impl EhciInterface {
    pub fn new(
        num_endpoints: u8,
        endpoints: &'static mut [EhciEndpoint],
        interface_class: u8,
        interface_sub_class: u8,
        interface_protocol: u8,
        i_interface: u8,
    ) -> Self {
        return Self {
            num_endpoints,
            endpoints,
            interface_class,
            interface_sub_class,
            interface_protocol,
            i_interface,
        };
    }

    pub fn from_raw(r#in: &UsbInterfaceDescriptor, endpoints: &'static mut [EhciEndpoint]) -> Self {
        return Self {
            num_endpoints: r#in.b_num_endpoints,
            endpoints: endpoints,
            interface_class: r#in.b_interface_class,
            interface_sub_class: r#in.b_interface_sub_class,
            interface_protocol: r#in.b_interface_protocol,
            i_interface: r#in.b_interface_protocol,
        };
    }
}

impl UsbInterface for EhciInterface {
    fn endpoint_count(&self) -> u16 {
        return self.num_endpoints as u16;
    }
    fn get_class(&self) -> u8 {
        return self.interface_class;
    }
    fn get_sub_class(&self) -> u8 {
        return self.interface_sub_class;
    }
    fn get_protocol(&self) -> u8 {
        return self.interface_protocol;
    }
    fn get_endpoint(&self, index: u16) -> Option<&dyn UsbEndpoint> {
        if index > self.num_endpoints as u16 {
            return Option::None;
        }
        return Option::Some(&self.endpoints[index as usize]);
    }
    fn get_mut_endpoint(&mut self, index: u16) -> Option<&mut dyn UsbEndpoint> {
        if index > self.num_endpoints as u16 {
            return Option::None;
        }
        return Option::Some(&mut self.endpoints[index as usize]);
    }
}

impl Default for EhciInterface {
    fn default() -> Self {
        return Self {
            num_endpoints: 0,
            endpoints: invalid_mut_slice(),
            interface_class: 0,
            interface_sub_class: 0,
            interface_protocol: 0,
            i_interface: 0,
        };
    }
}
