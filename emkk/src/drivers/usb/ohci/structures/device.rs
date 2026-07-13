use core::{ffi::c_void, ptr};

use crate::drivers::usb::{
    independent::{UsbDeviceInformation, UsbDeviceState, UsbTransferType},
    ohci::{
        configuration_parser::OhciDeviceConfiguration,
        structures::{
            endpoint::{
                EndpointDescriptorBitPart, OhciEndpointDescriptor, OhciNonPeriodicEndpoint,
            },
            non_periodic_list::OhciNonPeriodicList,
        },
    },
    standard_requests::{UsbDescriptor, UsbDeviceStandardRequest, UsbStandardDeviceRequest},
    traits::{UsbDevice, UsbDeviceExtendedRequest},
};

pub struct OhciDevice {
    pub(in crate::drivers::usb::ohci) device_address: u8,
    port: u8,
    class_code: u8,
    sub_class_code: u8,
    pub(in crate::drivers::usb::ohci) state: UsbDeviceState,
    pub(in crate::drivers::usb::ohci) control_ep: OhciNonPeriodicEndpoint,
    control_list: &'static OhciNonPeriodicList,
    signaled: bool,
    pub(in crate::drivers::usb::ohci) device_information: UsbDeviceInformation,
    pub(in crate::drivers::usb::ohci) configuration: OhciDeviceConfiguration,
}

impl OhciDevice {
    pub fn new_resetted(
        port: u8,
        control_ep: OhciEndpointDescriptor,
        control_list: &'static OhciNonPeriodicList,
    ) -> Self {
        return Self {
            device_address: 0,
            port,
            class_code: 0,
            sub_class_code: 0,
            state: UsbDeviceState::Resetted,
            control_ep: OhciNonPeriodicEndpoint::new(UsbTransferType::Control, control_ep),
            control_list,
            signaled: false,
            device_information: UsbDeviceInformation {
                device_class: 0,
                device_sub_class: 0,
                device_protocol: 0,
                vendor_id: 0,
                product_id: 0,
                manufacturer: 0,
                i_product: 0,
                serial_number: 0,
                max_power_ma: 0,
                num_interfaces: 0,
            },
            configuration: OhciDeviceConfiguration::empty(),
        };
    }
    /**NOTICE: This is kinda illegal, since it could be that a &mut OhciDevice modifies another &mut OhciDevice reference*/
    pub fn signal(&mut self) {
        self.signaled = true;
    }

    pub fn new_detached(
        port: u8,
        control_ep: OhciEndpointDescriptor,
        control_list: &'static OhciNonPeriodicList,
    ) -> Self {
        return Self {
            device_address: 0,
            port,
            class_code: 0,
            sub_class_code: 0,
            state: UsbDeviceState::Detached,
            control_ep: OhciNonPeriodicEndpoint::new(UsbTransferType::Control, control_ep),
            control_list,
            signaled: false,
            device_information: UsbDeviceInformation {
                device_class: 0,
                device_sub_class: 0,
                device_protocol: 0,
                vendor_id: 0,
                product_id: 0,
                manufacturer: 0,
                i_product: 0,
                serial_number: 0,
                max_power_ma: 0,
                num_interfaces: 0,
            },
            configuration: OhciDeviceConfiguration::empty(),
        };
    }

    pub fn control_without_data(&mut self, request: &UsbStandardDeviceRequest) {
        self.control_ep.send_setup_status(
            0,
            ptr::from_ref(request) as u32,
            UsbStandardDeviceRequest::SIZE,
            0,
            1,
            true,
        );
        self.control_list.send_for_processing();
    }

    pub fn control_with_data_to_host(
        &mut self,
        request: &UsbStandardDeviceRequest,
        data_ptr: u32,
        data_length: u32,
        exact_fit: bool,
    ) {
        assert_ne!(data_length, 0);
        self.control_ep.send_setup_status(
            0,
            ptr::from_ref(request) as u32,
            UsbStandardDeviceRequest::SIZE,
            data_ptr,
            data_length,
            exact_fit,
        );
        self.control_list.send_for_processing();
    }

    pub fn control_with_data_from_host(
        &mut self,
        request: &UsbStandardDeviceRequest,
        data_ptr: u32,
        data_length: u32,
        exact_fit: bool,
    ) {
        assert_ne!(data_length, 0);
        self.control_ep.send_setup_out_status(
            0,
            ptr::from_ref(request) as u32,
            UsbStandardDeviceRequest::SIZE,
            data_ptr,
            data_length,
            exact_fit,
        );
        self.control_list.send_for_processing();
    }

    pub fn await_interrupt(&mut self) {
        while !self.signaled {}
        self.signaled = false;
    }
}

impl UsbDevice for OhciDevice {
    fn detach(&mut self) {
        self.state = UsbDeviceState::Detached;
        self.control_ep
            .get_endpoint_descriptor()
            .set(EndpointDescriptorBitPart::K, true);
        self.control_ep
            .get_endpoint_descriptor()
            .set(EndpointDescriptorBitPart::Dum, true);
        todo!()
    }
    fn device_address(&self) -> u8 {
        return self.device_address;
    }
    fn get_class_code(&self) -> u8 {
        return self.class_code;
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
    fn get_state(&self) -> crate::drivers::usb::independent::UsbDeviceState {
        return self.state;
    }
    fn get_sub_class_code(&self) -> u8 {
        return self.sub_class_code;
    }
}
