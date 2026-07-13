use crate::drivers::usb::{
    ohci::structures::endpoint::{
        EndpointDescriptorBitPart::S, OhciGeneralEndpoint, OhciGeneralEndpointRealEndpoint,
    },
    standard_requests::UsbInterfaceDescriptor,
    traits::UsbInterface,
};

pub struct OhciInterface {
    interface_number: u8,
    alternate_setting: u8,
    interface_class: u8,
    interface_sub_class: u8,
    interface_protocol: u8,
    pub(in crate::drivers::usb::ohci) endpoints: &'static mut [OhciGeneralEndpoint],
}

impl OhciInterface {
    pub fn from_raw(
        descriptor: &UsbInterfaceDescriptor,
        endpoints: &'static mut [OhciGeneralEndpoint],
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

impl UsbInterface for OhciInterface {
    fn endpoint_count(&self) -> u16 {
        return self.endpoints.len() as u16;
    }
    fn get_class(&self) -> u8 {
        return self.interface_class;
    }
    fn get_sub_class(&self) -> u8 {
        return self.interface_sub_class;
    }
    fn get_endpoint(&self, index: u16) -> Option<&dyn crate::drivers::usb::traits::UsbEndpoint> {
        if index as usize > self.endpoints.len() {
            return Option::None;
        }
        return match &self.endpoints[index as usize].real_endpoint {
            OhciGeneralEndpointRealEndpoint::Unassigned => Option::None,
            OhciGeneralEndpointRealEndpoint::Periodic(ep) => Option::Some(*ep),
            OhciGeneralEndpointRealEndpoint::NonPeriodic(ep) => Option::Some(*ep),
        };
    }
    fn get_mut_endpoint(
        &mut self,
        index: u16,
    ) -> Option<&mut dyn crate::drivers::usb::traits::UsbEndpoint> {
        if index as usize > self.endpoints.len() {
            return Option::None;
        }
        return match &mut self.endpoints[index as usize].real_endpoint {
            OhciGeneralEndpointRealEndpoint::Unassigned => Option::None,
            OhciGeneralEndpointRealEndpoint::Periodic(ep) => Option::Some(*ep),
            OhciGeneralEndpointRealEndpoint::NonPeriodic(ep) => Option::Some(*ep),
        };
    }
    fn get_protocol(&self) -> u8 {
        return self.interface_protocol;
    }
}
