use core::ptr::null_mut;

use crate::drivers::usb::{
    ehci::structures::endpoint::EhciEndpoint,
    traits::{UsbEndpoint, UsbInterface},
};

pub struct EhciInterface {
    num_endpoints: u8,
    endpoints: *mut EhciEndpoint,
    interface_class: u8,
    interface_sub_class: u8,
    interface_protocol: u8,
    i_interface: u8,
}

impl EhciInterface {
    pub fn new(
        num_endpoints: u8,
        endpoints: *mut EhciEndpoint,
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

    pub fn get_endpoints(&self) -> *mut EhciEndpoint {
        return self.endpoints;
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

        unsafe {
            let ptr = self.endpoints.add(index as usize);
            let ret = &*ptr;
            return Option::Some(ret);
        }
    }
}

impl Default for EhciInterface {
    fn default() -> Self {
        return Self {
            num_endpoints: 0,
            endpoints: null_mut(),
            interface_class: 0,
            interface_sub_class: 0,
            interface_protocol: 0,
            i_interface: 0,
        };
    }
}
