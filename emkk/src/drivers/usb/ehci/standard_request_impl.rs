use core::ffi::c_void;

use crate::{
    drivers::usb::{
        ehci::{EHCI_CONTROLLER, structures::device::EhciDevice},
        independent::{
            CLEAR_FEATURE_REQUEST, GET_CONFIGURATION_REQUEST, GET_DESCRIPTOR_REQUEST,
            GET_INTERFACE_REQUEST, GET_STATUS_REQUEST, SET_ADDRESS_REQUEST,
            SET_CONFIGURATION_REQUEST, SET_FEATURE_REQUEST, SET_INTERFACE_REQUEST,
            SYNCH_FRAME_REQUEST, UsbDeviceConfiguration, UsbEndpointFrameNumber, UsbGeneralStatus,
            UsbInterfaceAlternateSetting, UsbRecipient,
        },
        standard_requests::{UsbDescriptor, UsbDeviceStandardRequest},
        traits::{UsbDevice, UsbEndpoint},
    },
    hal::print::simple_kernel_panic,
    utils::traits::Region,
};

impl UsbDeviceStandardRequest for EhciDevice {
    fn clear_feature(&mut self, feature_selector: u16, recipient: UsbRecipient) {
        let descriptor = self.request_packet_address();
        match recipient {
            UsbRecipient::Zero => {
                descriptor.w_index = 0;
                descriptor.bm_request_type = 0;
            }
            UsbRecipient::Interface(interface) => {
                descriptor.w_index = interface as u16;
                descriptor.bm_request_type = 0b1;
            }
            UsbRecipient::Endpoint(endpoint) => {
                descriptor.w_index = endpoint as u16;
                descriptor.bm_request_type = 0b10;
            }
        }
        descriptor.w_value = feature_selector;
        descriptor.w_length = 0;
        descriptor.b_request = CLEAR_FEATURE_REQUEST;
        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );
        self.await_interrupt();
    }
    fn get_configuratuion(&mut self) -> UsbDeviceConfiguration {
        let descriptor = self.request_packet_address();
        descriptor.bm_request_type = 0b10000000;
        descriptor.b_request = GET_CONFIGURATION_REQUEST;
        descriptor.w_value = 0;
        descriptor.w_index = 0;
        descriptor.w_length = 1;

        let data_offset = self.default_control_endpoint_setup_data_packet_base + 16;

        self.default_control_endpoint.control_with_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
            data_offset,
            1,
            true,
        );
        self.await_interrupt();
        return unsafe { *(data_offset as *const u8) };
    }
    fn get_descriptor(
        &mut self,
        descriptor_type: u8,
        descriptor_index: u8,
        language_id: Option<u16>,
        descriptor_length: u16,
    ) -> UsbDescriptor {
        let descriptor = self.request_packet_address();
        descriptor.bm_request_type = 0b10000000;
        descriptor.b_request = GET_DESCRIPTOR_REQUEST;
        descriptor.w_value = (descriptor_type as u16) << 8 | descriptor_index as u16;
        descriptor.w_index = match language_id {
            Some(language_id) => language_id,
            None => 0,
        };
        descriptor.w_length = descriptor_length;
        #[allow(static_mut_refs)]
        let memory_base = match unsafe { EHCI_CONTROLLER.memory_space.alloc_zero(1) } {
            Ok(mb) => {
                if mb.get_base() > 0xFFFFFFFF {
                    simple_kernel_panic(
                        "EhciDevice/get_descriptor",
                        "Allocate memory for memory base is above 0xFFFFFFFF\n",
                    )
                }
                mb.get_base() as u32
            }
            Err(_e) => simple_kernel_panic(
                "EhciDevice/get_descriptor",
                "Could not allocate memory base for Descriptor\n",
            ),
        };
        self.default_control_endpoint.control_with_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
            memory_base,
            descriptor_length,
            true,
        );
        self.await_interrupt();
        return UsbDescriptor {
            descriptor_type,
            descriptor_index,
            descriptor_length,
            data: memory_base as *mut c_void,
        };
    }
    fn get_interface(&mut self, interface: u16) -> UsbInterfaceAlternateSetting {
        let descriptor = self.request_packet_address();

        descriptor.bm_request_type = 0b10000001;
        descriptor.b_request = GET_INTERFACE_REQUEST;
        descriptor.w_value = 0;
        descriptor.w_index = interface;
        descriptor.w_length = 1;

        let base_ptr = self.default_control_endpoint_setup_data_packet_base + 16;

        self.default_control_endpoint.control_with_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
            base_ptr,
            1,
            true,
        );
        self.await_interrupt();
        return unsafe { *(base_ptr as *const u8) };
    }
    fn get_status(&mut self, recipient: UsbRecipient) -> UsbGeneralStatus {
        let descriptor = self.request_packet_address();

        match recipient {
            UsbRecipient::Zero => {
                descriptor.w_index = 0;
                descriptor.bm_request_type = 0b10000000;
            }
            UsbRecipient::Interface(interface) => {
                descriptor.w_index = interface as u16;
                descriptor.bm_request_type = 0b10000001;
            }
            UsbRecipient::Endpoint(endpoint) => {
                descriptor.w_index = endpoint as u16;
                descriptor.bm_request_type = 0b10000010;
            }
        }
        descriptor.b_request = GET_STATUS_REQUEST;
        descriptor.w_value = 0;
        descriptor.w_length = 2;

        let base_ptr = self.default_control_endpoint_setup_data_packet_base + 16;

        self.default_control_endpoint.control_with_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
            base_ptr,
            2,
            true,
        );
        self.await_interrupt();
        return unsafe { *(base_ptr as *const u16) };
    }
    fn set_address(&mut self, device_address: u16) {
        let descriptor = self.request_packet_address();
        descriptor.bm_request_type = 0;
        descriptor.b_request = SET_ADDRESS_REQUEST;
        descriptor.w_value = device_address;
        descriptor.w_index = 0;
        descriptor.w_length = 0;

        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );

        self.await_interrupt();
    }
    fn set_configuration(&mut self, configuration: UsbDeviceConfiguration) {
        let descriptor = self.request_packet_address();
        descriptor.bm_request_type = 0;
        descriptor.b_request = SET_CONFIGURATION_REQUEST;
        descriptor.w_value = configuration as u16;
        descriptor.w_index = 0;
        descriptor.w_length = 0;
        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );
        self.await_interrupt();
    }
    fn set_descriptor(
        &mut self,
        _descriptor_type: u8,
        _descriptor_index: u8,
        _language_id: Option<u16>,
        _descriptor_length: u16,
        _in_descriptor: &UsbDescriptor,
    ) {
        simple_kernel_panic(
            "EhciDevice/set_descriptor",
            "Unimplemented due to it being optional\n",
        )
    }
    fn set_feature(&mut self, feature_selector: u16, test_selector: u8, recipient: UsbRecipient) {
        let descriptor = self.request_packet_address();
        match recipient {
            UsbRecipient::Zero => {
                descriptor.bm_request_type = 0;
                descriptor.w_index = 0 | (test_selector as u16) << 8;
            }
            UsbRecipient::Interface(interface) => {
                descriptor.bm_request_type = 0b1;
                descriptor.w_index = interface as u16;
            }
            UsbRecipient::Endpoint(endpoint) => {
                descriptor.bm_request_type = 0b10;
                descriptor.w_index = endpoint as u16;
            }
        }
        descriptor.b_request = SET_FEATURE_REQUEST;
        descriptor.w_value = feature_selector;
        descriptor.w_length = 0;
        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );
        self.await_interrupt();
    }
    fn set_interface(&mut self, alternate_setting: UsbInterfaceAlternateSetting, interface: u16) {
        let descriptor = self.request_packet_address();
        descriptor.bm_request_type = 1;
        descriptor.b_request = SET_INTERFACE_REQUEST;
        descriptor.w_value = alternate_setting as u16;
        descriptor.w_index = interface;
        descriptor.w_length = 0;
        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );
        self.await_interrupt();
    }

    fn synch_frame(&mut self, endpoint: u16) -> UsbEndpointFrameNumber {
        let descriptor = self.request_packet_address();
        descriptor.bm_request_type = 0b10000010;
        descriptor.b_request = SYNCH_FRAME_REQUEST;
        descriptor.w_value = 0;
        descriptor.w_index = endpoint;
        descriptor.w_length = 2;

        let base_ptr = self.default_control_endpoint_setup_data_packet_base + 16;
        self.default_control_endpoint.control_with_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
            base_ptr,
            2,
            true,
        );
        self.await_interrupt();
        return unsafe { *(base_ptr as *const u16) };
    }
}
