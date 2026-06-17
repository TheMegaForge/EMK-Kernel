use crate::drivers::usb::{
    independent::UsbDeviceState,
    standard_requests::UsbDeviceStandardRequest,
    traits::{UsbDevice, UsbDeviceExtendedRequest},
};

pub struct OhciDevice {
    device_address: u8,
    port: u8,
    class_code: u8,
    sub_class_code: u8,
    state: UsbDeviceState,
    control_list_ep_index: u8,
}

impl OhciDevice {
    pub fn new_resetted(port: u8, control_list_ep_index: u8) -> Self {
        return Self {
            device_address: 0,
            port,
            class_code: 0,
            sub_class_code: 0,
            state: UsbDeviceState::Resetted,
            control_list_ep_index,
        };
    }
    pub fn new_detached(port: u8, control_list_ep_index: u8) -> Self {
        return Self {
            device_address: 0,
            port,
            class_code: 0,
            sub_class_code: 0,
            state: UsbDeviceState::Detached,
            control_list_ep_index,
        };
    }
}

impl UsbDevice for OhciDevice {
    fn await_interrupt(&self) {
        todo!()
    }
    fn detach(&mut self) {
        todo!()
    }
    fn device_address(&self) -> u8 {
        return self.device_address;
    }
    fn endpoint_count(&self) -> u8 {
        todo!()
    }
    fn get_class_code(&self) -> u8 {
        return self.class_code;
    }
    fn get_configuration(
        &self,
        config: u8,
    ) -> Option<&dyn crate::drivers::usb::traits::UsbConfiguration> {
        todo!()
    }
    fn get_configuration_count(&self) -> u8 {
        todo!()
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
    fn request_packet_address<'a>(
        &'a self,
    ) -> &'a mut crate::drivers::usb::standard_requests::UsbStandardDeviceRequest {
        todo!()
    }
}

impl UsbDeviceStandardRequest for OhciDevice {
    fn clear_feature(
        &mut self,
        feature_selector: u16,
        recipient: crate::drivers::usb::independent::UsbRecipient,
    ) {
        todo!()
    }
    fn get_configuratuion(&mut self) -> crate::drivers::usb::independent::UsbDeviceConfiguration {
        todo!()
    }
    fn get_descriptor(
        &mut self,
        descriptor_type: u8,
        descriptor_index: u8,
        language_id: Option<u16>,
        descriptor_length: u16,
    ) -> crate::drivers::usb::standard_requests::UsbDescriptor {
        todo!()
    }
    fn get_interface(
        &mut self,
        interface: u16,
    ) -> crate::drivers::usb::independent::UsbInterfaceAlternateSetting {
        todo!()
    }
    fn get_status(
        &mut self,
        recipient: crate::drivers::usb::independent::UsbRecipient,
    ) -> crate::drivers::usb::independent::UsbGeneralStatus {
        todo!()
    }
    fn set_address(&mut self, device_address: u16) {
        todo!()
    }
    fn set_configuration(
        &mut self,
        configuration: crate::drivers::usb::independent::UsbDeviceConfiguration,
    ) {
        todo!()
    }
    fn set_descriptor(
        &mut self,
        descriptor_type: u8,
        descriptor_index: u8,
        language_id: Option<u16>,
        descriptor_length: u16,
        in_descriptor: &crate::drivers::usb::standard_requests::UsbDescriptor,
    ) {
        todo!()
    }
    fn set_feature(
        &mut self,
        feature_selector: u16,
        test_selector: u8,
        recipient: crate::drivers::usb::independent::UsbRecipient,
    ) {
        todo!()
    }
    fn set_interface(
        &mut self,
        alternate_setting: crate::drivers::usb::independent::UsbInterfaceAlternateSetting,
        interface: u16,
    ) {
        todo!()
    }
    fn synch_frame(
        &mut self,
        endpoint: u16,
    ) -> crate::drivers::usb::independent::UsbEndpointFrameNumber {
        todo!()
    }
}

impl UsbDeviceExtendedRequest for OhciDevice {
    fn set_protocol(&mut self, request: u8, interface: u16) {
        todo!()
    }
}
