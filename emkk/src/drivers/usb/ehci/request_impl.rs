use core::ffi::c_void;

use crate::{
    drivers::usb::{
        ehci::{EHCI_CONTROLLER, structures::device::EhciDevice},
        independent::{
            UsbDescriptorType, UsbDeviceConfiguration, UsbEndpointFrameNumber, UsbFeatureSelector,
            UsbGeneralStatus, UsbInterfaceAlternateSetting, UsbRecipient,
            UsbRequestCode::{self, GetConfiguration},
        },
        standard_requests::{UsbDescriptor, UsbDeviceStandardRequest},
        traits::{UsbDevice, UsbDeviceExtendedRequest, UsbEndpoint},
    },
    hal::{memory::allocator::MemoryBlock, print::simple_kernel_panic},
    utils::traits::Region,
};

impl UsbDeviceStandardRequest for EhciDevice {
    fn clear_feature(&mut self, feature_selector: UsbFeatureSelector, recipient: UsbRecipient) {
        let descriptor = self.request_packet_address();
        let w_index;
        let bm_request_type;
        match recipient {
            UsbRecipient::Zero => {
                w_index = 0;
                bm_request_type = 0;
            }
            UsbRecipient::Interface(interface) => {
                w_index = interface as u16;
                bm_request_type = 0b1;
            }
            UsbRecipient::Endpoint(endpoint) => {
                w_index = endpoint as u16;
                bm_request_type = 0b10;
            }
        }

        descriptor.set(
            bm_request_type,
            UsbRequestCode::ClearFeature,
            feature_selector as u16,
            w_index,
            0,
        );

        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );
        self.await_interrupt();
    }
    fn usb_request_get_configuration(&mut self) -> UsbDeviceConfiguration {
        let descriptor = self.request_packet_address();
        descriptor.set(0b10000000, UsbRequestCode::GetConfiguration, 0, 0, 1);

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
        descriptor_type: UsbDescriptorType,
        descriptor_index: u8,
        language_id: Option<u16>,
        descriptor_length: u16,
    ) -> UsbDescriptor {
        let descriptor = self.request_packet_address();
        descriptor.set(
            0b10000000,
            UsbRequestCode::GetConfiguration,
            (descriptor_type as u16) << 8 | descriptor_index as u16,
            match language_id {
                Some(language_id) => language_id,
                None => 0,
            },
            descriptor_length,
        );
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
            data: MemoryBlock::new(0x1000, memory_base as u64),
        };
    }
    fn get_interface(&mut self, interface: u16) -> UsbInterfaceAlternateSetting {
        let descriptor = self.request_packet_address();
        descriptor.set(0b10000001, UsbRequestCode::GetInterface, 0, interface, 1);

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
        let w_index;
        let bm_request_type;
        match recipient {
            UsbRecipient::Zero => {
                w_index = 0;
                bm_request_type = 0b10000000;
            }
            UsbRecipient::Interface(interface) => {
                w_index = interface as u16;
                bm_request_type = 0b10000001;
            }
            UsbRecipient::Endpoint(endpoint) => {
                w_index = endpoint as u16;
                bm_request_type = 0b10000010;
            }
        }

        descriptor.set(bm_request_type, UsbRequestCode::GetStatus, 0, w_index, 2);

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
        descriptor.set(0, UsbRequestCode::SetAddress, device_address, 0, 0);

        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );

        self.await_interrupt();
    }
    fn set_configuration(&mut self, configuration: UsbDeviceConfiguration) {
        let descriptor = self.request_packet_address();
        descriptor.set(
            0,
            UsbRequestCode::SetConfiguration,
            configuration as u16,
            0,
            0,
        );

        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );
        self.await_interrupt();
    }
    fn set_descriptor(
        &mut self,
        _descriptor_type: UsbDescriptorType,
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
    fn set_feature(
        &mut self,
        feature_selector: UsbFeatureSelector,
        test_selector: u8,
        recipient: UsbRecipient,
    ) {
        let descriptor = self.request_packet_address();
        let bm_request_type;
        let w_index;
        match recipient {
            UsbRecipient::Zero => {
                bm_request_type = 0;
                w_index = 0 | (test_selector as u16) << 8;
            }
            UsbRecipient::Interface(interface) => {
                bm_request_type = 0b1;
                w_index = interface as u16;
            }
            UsbRecipient::Endpoint(endpoint) => {
                bm_request_type = 0b10;
                w_index = endpoint as u16;
            }
        }
        descriptor.set(
            bm_request_type,
            UsbRequestCode::SetFeature,
            feature_selector as u16,
            w_index,
            0,
        );
        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );
        self.await_interrupt();
    }
    fn set_interface(&mut self, alternate_setting: UsbInterfaceAlternateSetting, interface: u16) {
        let descriptor = self.request_packet_address();
        descriptor.set(
            1,
            UsbRequestCode::SetInterface,
            alternate_setting as u16,
            interface,
            0,
        );
        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );
        self.await_interrupt();
    }

    fn synch_frame(&mut self, endpoint: u16) -> UsbEndpointFrameNumber {
        let descriptor = self.request_packet_address();
        descriptor.set(0b10000010, UsbRequestCode::SynchFrame, 0, endpoint, 2);

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

impl UsbDeviceExtendedRequest for EhciDevice {
    fn set_protocol(&mut self, request: u8, w_value: u16, interface: u16) {
        let descriptor = self.request_packet_address();
        descriptor.bm_request_type = 0x21;
        descriptor.b_request = request;
        descriptor.w_value = w_value;
        descriptor.w_index = interface;
        descriptor.w_length = 0;
        self.default_control_endpoint.control_without_data(
            self.default_control_endpoint
                .get_designated_queue_head_address() as *mut c_void,
            self.default_control_endpoint_setup_data_packet_base,
        );
        self.await_interrupt();
    }
}
