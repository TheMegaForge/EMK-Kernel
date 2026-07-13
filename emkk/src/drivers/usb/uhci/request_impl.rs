use crate::{
    drivers::usb::{
        independent::{
            UsbRecipient::{self, Zero},
            UsbRequestCode,
        },
        standard_requests::{UsbDescriptor, UsbDeviceStandardRequest, UsbStandardDeviceRequest},
        traits::UsbDeviceExtendedRequest,
        uhci::{UHCI_CONTROLLER, structures::device::UhciDevice},
    },
    time::sleep,
};

impl UsbDeviceStandardRequest for UhciDevice {
    fn clear_feature(
        &mut self,
        feature_selector: crate::drivers::usb::independent::UsbFeatureSelector,
        recipient: crate::drivers::usb::independent::UsbRecipient,
    ) {
        let bm_request;
        let w_index;
        match recipient {
            UsbRecipient::Zero => {
                bm_request = 0;
                w_index = 0
            }
            UsbRecipient::Interface(interface) => {
                bm_request = 1;
                w_index = interface as u16;
            }
            UsbRecipient::Endpoint(endpoint) => {
                bm_request = 0b10;
                w_index = endpoint as u16;
            }
        }
        let request =
            UsbStandardDeviceRequest::new(bm_request, UsbRequestCode::ClearFeature, 0, w_index, 1);
        self.control_endpoint
            .send_control_without_data(&raw const request, true);
        self.await_interrupt();
    }
    fn get_descriptor(
        &mut self,
        descriptor_type: crate::drivers::usb::independent::UsbDescriptorType,
        descriptor_index: u8,
        language_id: Option<u16>,
        descriptor_length: u16,
    ) -> crate::drivers::usb::standard_requests::UsbDescriptor {
        let request = UsbStandardDeviceRequest::new(
            0b1000_0000,
            UsbRequestCode::GetDescriptor,
            ((descriptor_type as u16) << 8) | descriptor_index as u16,
            match language_id {
                Some(id) => id,
                None => 0,
            },
            descriptor_length,
        );
        #[allow(static_mut_refs)]
        let mb = unsafe {
            UHCI_CONTROLLER
                .private_physical_memory
                .alloc_zero(1)
                .unwrap()
        };
        assert!(1024 >= descriptor_length);
        self.control_endpoint.send_control_read(
            &raw const request,
            mb.base as u32,
            descriptor_length,
            true,
        );
        self.await_interrupt();
        return UsbDescriptor {
            descriptor_type,
            descriptor_index,
            descriptor_length,
            data: mb,
        };
    }
    fn get_interface(
        &mut self,
        interface: u16,
    ) -> crate::drivers::usb::independent::UsbInterfaceAlternateSetting {
        let request = UsbStandardDeviceRequest::new(
            0b1000_0000,
            UsbRequestCode::GetInterface,
            0,
            interface,
            1,
        );
        let mut ret = 0u8;
        self.control_endpoint
            .send_control_read(&raw const request, &raw mut ret as u32, 1, true);
        self.await_interrupt();
        return ret;
    }
    fn get_status(
        &mut self,
        recipient: crate::drivers::usb::independent::UsbRecipient,
    ) -> crate::drivers::usb::independent::UsbGeneralStatus {
        let bm_request;
        let w_index;
        match recipient {
            UsbRecipient::Zero => {
                bm_request = 0b1000_0000;
                w_index = 0;
            }
            UsbRecipient::Interface(interface) => {
                bm_request = 0b1000_0001;
                w_index = interface as u16;
            }
            UsbRecipient::Endpoint(endpoint) => {
                bm_request = 0b1000_0010;
                w_index = endpoint as u16;
            }
        }

        let request =
            UsbStandardDeviceRequest::new(bm_request, UsbRequestCode::GetStatus, 0, w_index, 2);
        let mut ret = 0;
        self.control_endpoint
            .send_control_read(&raw const request, &raw mut ret as u32, 2, true);
        self.await_interrupt();
        return ret;
    }
    fn set_address(&mut self, device_address: u16) {
        let request =
            UsbStandardDeviceRequest::new(0, UsbRequestCode::SetAddress, device_address, 0, 0);

        self.control_endpoint
            .send_control_without_data(&raw const request, true);
        self.await_interrupt();
    }
    fn set_configuration(
        &mut self,
        configuration: crate::drivers::usb::independent::UsbDeviceConfiguration,
    ) {
        let request = UsbStandardDeviceRequest::new(
            0,
            UsbRequestCode::SetConfiguration,
            configuration as u16,
            0,
            0,
        );
        self.control_endpoint
            .send_control_without_data(&raw const request, true);
        self.await_interrupt();
    }
    fn set_descriptor(
        &mut self,
        descriptor_type: crate::drivers::usb::independent::UsbDescriptorType,
        descriptor_index: u8,
        language_id: Option<u16>,
        descriptor_length: u16,
        in_descriptor: &crate::drivers::usb::standard_requests::UsbDescriptor,
    ) {
        assert!(1024 >= in_descriptor.descriptor_length);
        assert_eq!(descriptor_length, in_descriptor.descriptor_length);
        let request = UsbStandardDeviceRequest::new(
            0,
            UsbRequestCode::SetDescriptor,
            (descriptor_type as u16) << 8 | descriptor_index as u16,
            match language_id {
                Some(id) => id,
                None => 0,
            },
            descriptor_length,
        );
        self.control_endpoint.send_control_write(
            &raw const request,
            in_descriptor.data.base as u32,
            in_descriptor.descriptor_length,
            true,
        );
        self.await_interrupt();
    }
    fn set_feature(
        &mut self,
        feature_selector: crate::drivers::usb::independent::UsbFeatureSelector,
        _test_selector: u8,
        recipient: crate::drivers::usb::independent::UsbRecipient,
    ) {
        let bm_request;
        let w_index;

        match recipient {
            Zero => {
                bm_request = 0;
                w_index = 0;
            }
            UsbRecipient::Interface(interface) => {
                bm_request = 1;
                w_index = interface as u16;
            }
            UsbRecipient::Endpoint(endpoint) => {
                bm_request = 0b10;
                w_index = endpoint as u16;
            }
        }
        let request = UsbStandardDeviceRequest::new(
            bm_request,
            UsbRequestCode::SetFeature,
            feature_selector as u16,
            w_index,
            0,
        );
        self.control_endpoint
            .send_control_without_data(&raw const request, true);
        self.await_interrupt();
    }
    fn set_interface(
        &mut self,
        alternate_setting: crate::drivers::usb::independent::UsbInterfaceAlternateSetting,
        interface: u16,
    ) {
        let request = UsbStandardDeviceRequest::new(
            1,
            UsbRequestCode::SetInterface,
            alternate_setting as u16,
            interface,
            0,
        );
        self.control_endpoint
            .send_control_without_data(&raw const request, true);
        self.await_interrupt();
    }
    fn synch_frame(
        &mut self,
        endpoint: u16,
    ) -> crate::drivers::usb::independent::UsbEndpointFrameNumber {
        let request =
            UsbStandardDeviceRequest::new(0b1000_0010, UsbRequestCode::SynchFrame, 0, endpoint, 2);
        let mut ret = 0;

        self.control_endpoint
            .send_control_read(&raw const request, &raw mut ret as u32, 2, true);
        self.await_interrupt();
        return ret;
    }
    fn usb_request_get_configuration(
        &mut self,
    ) -> crate::drivers::usb::independent::UsbDeviceConfiguration {
        let request =
            UsbStandardDeviceRequest::new(0b1000_0000, UsbRequestCode::GetConfiguration, 0, 0, 1);
        let mut ret = 0u8;
        self.control_endpoint
            .send_control_read(&raw const request, &raw mut ret as u32, 1, true);
        self.await_interrupt();
        return ret;
    }
}

impl UsbDeviceExtendedRequest for UhciDevice {
    fn set_protocol(&mut self, request: u8, w_value: u16, interface: u16) {
        let request = UsbStandardDeviceRequest {
            bm_request_type: 0x21,
            b_request: request,
            w_value: w_value,
            w_index: interface,
            w_length: 0,
        };
        self.control_endpoint
            .send_control_without_data(&raw const request, true);
        self.await_interrupt();
    }
}
