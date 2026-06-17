use core::ffi::c_void;

use crate::drivers::usb::independent::{
    UsbDeviceConfiguration, UsbEndpointFrameNumber, UsbGeneralStatus, UsbInterfaceAlternateSetting,
    UsbRecipient,
};

pub struct UsbDescriptor {
    pub descriptor_type: u8,
    pub descriptor_index: u8,
    pub descriptor_length: u16,
    pub data: *mut c_void,
}

#[repr(C, packed)]
pub struct UsbDefaultDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
}

#[repr(C, packed)]
pub struct UsbDeviceDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub bcd_usb: u16,
    pub b_device_class: u8,
    pub b_device_sub_class: u8,
    pub b_device_protocol: u8,
    pub b_max_packet_size0: u8,
    pub id_vendor: u16,
    pub id_product: u16,
    pub bcd_device: u16,
    pub i_manufacturer: u8,
    pub i_product: u8,
    pub i_serial_number: u8,
    pub b_num_configurations: u8,
}
#[repr(C, packed)]
pub struct UsbDeviceQualifierDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub bcd_usb: u16,
    pub b_device_class: u8,
    pub b_device_sub_class: u8,
    pub b_device_protocol: u8,
    pub b_max_packet_size0: u8,
    pub b_num_configurations: u8,
    pub b_reserved: u8,
}
#[repr(C, packed)]
pub struct UsbConfigurationDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub w_total_length: u16,
    pub b_num_interfaces: u8,
    pub b_configuration_value: u8,
    pub i_configuration: u8,
    pub bm_attributes: u8,
    pub b_max_power: u8,
}
#[repr(C, packed)]
pub struct UsbOtherSpeedConfigurationDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub w_total_length: u16,
    pub b_num_interfaces: u8,
    pub b_configuration_value: u8,
    pub i_configuration: u8,
    pub bm_attributes: u8,
    pub b_max_power: u8,
}
#[repr(C, packed)]
pub struct UsbInterfaceDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_interface_number: u8,
    pub b_alternate_setting: u8,
    pub b_num_endpoints: u8,
    pub b_interface_class: u8,
    pub b_interface_sub_class: u8,
    pub b_interface_protocol: u8,
    pub i_interface: u8,
}
#[repr(C, packed)]
pub struct UsbEndpointDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_endpoint_address: u8,
    pub bm_attributes: u8,
    pub w_max_packet_size: u16,
    pub b_interval: u8,
}
#[repr(C, packed)]
pub struct UsbHIDDescriptor {
    pub b_length: u8,
    pub b_descriptor_type0: u8,
    pub bcd_hid: u16,
    pub b_country_code: u8,
    pub b_num_descriptors: u8,
    pub b_descriptor_type1: u8,
    pub w_descriptor_length: u16,
}

pub struct UsbHID {
    pub bcd_hid: u16,
    pub country_code: u8,
    pub num_descriptors: u8,
    pub descriptor_type: u8,
    pub descriptor_length: u16,
    pub interface_index: u8,
}

impl UsbDescriptor {
    pub fn as_device_descriptor(&self) -> &UsbDeviceDescriptor {
        return unsafe { (self.data as *const UsbDeviceDescriptor).as_ref().unwrap() };
    }
    pub fn as_device_qualifier_descriptor(&self) -> &UsbDeviceQualifierDescriptor {
        return unsafe {
            (self.data as *const UsbDeviceQualifierDescriptor)
                .as_ref()
                .unwrap()
        };
    }
    pub fn as_configuration_descriptor(&self) -> &UsbConfigurationDescriptor {
        return unsafe {
            (self.data as *const UsbConfigurationDescriptor)
                .as_ref()
                .unwrap()
        };
    }
    pub fn as_other_speed_configuration_descriptor(&self) -> &UsbOtherSpeedConfigurationDescriptor {
        return unsafe {
            (self.data as *const UsbOtherSpeedConfigurationDescriptor)
                .as_ref()
                .unwrap()
        };
    }
    pub fn as_interface_descriptor(&self) -> &UsbInterfaceDescriptor {
        return unsafe {
            (self.data as *const UsbInterfaceDescriptor)
                .as_ref()
                .unwrap()
        };
    }
    pub fn as_endpoint_descriptor(&self) -> &UsbEndpointDescriptor {
        return unsafe {
            (self.data as *const UsbEndpointDescriptor)
                .as_ref()
                .unwrap()
        };
    }
}
#[repr(C, packed)]
pub struct UsbStandardDeviceRequest {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
}

pub trait UsbDeviceStandardRequest {
    fn clear_feature(&mut self, feature_selector: u16, recipient: UsbRecipient);
    fn get_configuratuion(&mut self) -> UsbDeviceConfiguration;
    fn get_descriptor(
        &mut self,
        descriptor_type: u8,
        descriptor_index: u8,
        language_id: Option<u16>,
        descriptor_length: u16,
    ) -> UsbDescriptor;
    fn get_interface(&mut self, interface: u16) -> UsbInterfaceAlternateSetting;
    fn get_status(&mut self, recipient: UsbRecipient) -> UsbGeneralStatus;
    fn set_address(&mut self, device_address: u16);
    fn set_configuration(&mut self, configuration: UsbDeviceConfiguration);
    fn set_descriptor(
        &mut self,
        descriptor_type: u8,
        descriptor_index: u8,
        language_id: Option<u16>,
        descriptor_length: u16,
        in_descriptor: &UsbDescriptor,
    );
    fn set_feature(&mut self, feature_selector: u16, test_selector: u8, recipient: UsbRecipient);
    fn set_interface(&mut self, alternate_setting: UsbInterfaceAlternateSetting, interface: u16);
    fn synch_frame(&mut self, endpoint: u16) -> UsbEndpointFrameNumber;
}
