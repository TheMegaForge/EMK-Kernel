use core::ffi;

use crate::{
    drivers::usb::{
        independent::UsbRequestCode,
        standard_requests::{UsbDescriptor, UsbDeviceStandardRequest, UsbStandardDeviceRequest},
        traits::UsbDeviceExtendedRequest,
        xhci::{XHCI_CONTROLLER, structures::device::XhciDevice},
    },
    hal::print::simple_kernel_panic,
    utils::memory::alloc_zero_or_crash,
};

impl UsbDeviceStandardRequest for XhciDevice {
    fn clear_feature(
        &mut self,
        feature_selector: crate::drivers::usb::independent::UsbFeatureSelector,
        recipient: crate::drivers::usb::independent::UsbRecipient,
    ) {
        let bm_request_type;
        let w_index;
        match recipient {
            crate::drivers::usb::independent::UsbRecipient::Zero => {
                bm_request_type = 0;
                w_index = 0;
            }
            crate::drivers::usb::independent::UsbRecipient::Interface(interface) => {
                bm_request_type = 1;
                w_index = interface as u16;
            }
            crate::drivers::usb::independent::UsbRecipient::Endpoint(endpoint) => {
                bm_request_type = 2;
                w_index = endpoint as u16;
            }
        }
        let request = UsbStandardDeviceRequest {
            bm_request_type: bm_request_type,
            b_request: UsbRequestCode::ClearFeature as u8,
            w_value: feature_selector as u16,
            w_index: w_index,
            w_length: 0,
        };
        self.control_endpoint
            .send_control_no_data(&request, 0, true);
        self.await_interrupt();
    }
    fn get_descriptor(
        &mut self,
        descriptor_type: crate::drivers::usb::independent::UsbDescriptorType,
        descriptor_index: u8,
        language_id: Option<u16>,
        descriptor_length: u16,
    ) -> crate::drivers::usb::standard_requests::UsbDescriptor {
        let data = alloc_zero_or_crash(
            #[allow(static_mut_refs)]
            unsafe {
                &mut XHCI_CONTROLLER.private_memory
            },
            1,
            #[allow(static_mut_refs)]
            unsafe {
                &mut XHCI_CONTROLLER.module
            },
            "Could not allocate Usb Descriptor\n",
        );
        let request = UsbStandardDeviceRequest {
            bm_request_type: 0b10000000,
            b_request: UsbRequestCode::GetDescriptor as u8,
            w_value: (descriptor_type as u16) << 8,
            w_index: match language_id {
                Some(id) => id,
                None => 0,
            },
            w_length: descriptor_length,
        };
        self.control_endpoint.send_control_read(
            &request,
            0,
            data.base,
            descriptor_length as u32,
            true,
        );
        self.await_interrupt();
        return UsbDescriptor {
            descriptor_type,
            descriptor_index,
            descriptor_length,
            data,
        };
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
        simple_kernel_panic("XhciDevice/set_address", "Not supported\n");
    }
    fn set_configuration(
        &mut self,
        configuration: crate::drivers::usb::independent::UsbDeviceConfiguration,
    ) {
        let request = UsbStandardDeviceRequest {
            bm_request_type: 0,
            b_request: UsbRequestCode::SetConfiguration as u8,
            w_value: configuration as u16,
            w_index: 0,
            w_length: 0,
        };
        self.control_endpoint
            .send_control_no_data(&request, 0, true);
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
        todo!()
    }
    fn set_feature(
        &mut self,
        feature_selector: crate::drivers::usb::independent::UsbFeatureSelector,
        test_selector: u8,
        recipient: crate::drivers::usb::independent::UsbRecipient,
    ) {
        _ = test_selector;
        let bm_request_type;
        let w_index;
        match recipient {
            crate::drivers::usb::independent::UsbRecipient::Zero => {
                bm_request_type = 0;
                w_index = 0;
            }
            crate::drivers::usb::independent::UsbRecipient::Interface(interface) => {
                bm_request_type = 1;
                w_index = interface as u16;
            }
            crate::drivers::usb::independent::UsbRecipient::Endpoint(endpoint) => {
                bm_request_type = 2;
                w_index = endpoint as u16;
            }
        }
        let request = UsbStandardDeviceRequest {
            bm_request_type,
            b_request: UsbRequestCode::SetFeature as u8,
            w_value: feature_selector as u16,
            w_index,
            w_length: 0,
        };
        self.control_endpoint
            .send_control_no_data(&request, 0, true);
        self.await_interrupt();
    }
    fn set_interface(
        &mut self,
        alternate_setting: crate::drivers::usb::independent::UsbInterfaceAlternateSetting,
        interface: u16,
    ) {
        let request = UsbStandardDeviceRequest {
            bm_request_type: 1,
            b_request: UsbRequestCode::SetInterface as u8,
            w_value: alternate_setting as u16,
            w_index: interface,
            w_length: 0,
        };
        self.control_endpoint
            .send_control_no_data(&request, 0, true);
        self.await_interrupt();
    }
    fn synch_frame(
        &mut self,
        endpoint: u16,
    ) -> crate::drivers::usb::independent::UsbEndpointFrameNumber {
        todo!()
    }
    fn usb_request_get_configuration(
        &mut self,
    ) -> crate::drivers::usb::independent::UsbDeviceConfiguration {
        todo!()
    }
}

impl UsbDeviceExtendedRequest for XhciDevice {
    fn set_protocol(&mut self, request: u8, w_value: u16, interface: u16) {
        let request = UsbStandardDeviceRequest {
            bm_request_type: 0x21,
            b_request: request,
            w_value: w_value,
            w_index: interface,
            w_length: 0,
        };
        self.control_endpoint
            .send_control_no_data(&request, 0, true);
        self.await_interrupt();
    }
}
