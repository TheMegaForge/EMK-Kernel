use core::ffi::c_void;

use crate::{
    drivers::usb::{
        independent::{
            Direction, UsbControllerType, UsbDeviceState, UsbEndpointError, UsbTransferType,
        },
        standard_requests::{UsbDeviceStandardRequest, UsbHID, UsbStandardDeviceRequest},
    },
    hal::{memory::allocator::Allocator, pci_bus::PciBus},
};

pub trait UsbEndpoint {
    // base should be used for qTDs
    fn set_address_and_length(&mut self, base: *mut c_void, maximum_qtds: u8);

    fn get_designated_queue_head_address(&self) -> u32;

    fn control_without_data(&mut self, qh_address: *mut c_void, status_page_buffer0_address: u32);
    // data_in : true => host receives data from device
    // data_in : false => device receives data from host
    fn control_with_data(
        &mut self,
        qh_address: *const c_void,
        setup_page_buffer0_address: u32,
        base_ptr: u32,
        ptr_length: u16,
        data_in: bool,
    ) -> Option<UsbEndpointError>;

    fn get_maximum_packet_size(&self) -> u16;
    fn update_max_packet_size(&mut self, max_packet_size: u16);

    fn endpoint_number(&self) -> u8;
    fn get_transfer_type(&self) -> UsbTransferType;
    fn get_direction(&self) -> Direction;
    fn calculate_interval_micro_frames(&self) -> u16;
}

pub trait UsbInterface {
    fn endpoint_count(&self) -> u16;
    fn get_class(&self) -> u8;
    fn get_sub_class(&self) -> u8;
    fn get_protocol(&self) -> u8;

    fn get_endpoint(&self, index: u16) -> Option<&dyn UsbEndpoint>;
    fn get_mut_endpoint(&mut self, index: u16) -> Option<&mut dyn UsbEndpoint>;
}

pub trait UsbConfiguration {
    fn get_interface_count(&self) -> u8;
    fn get_interface(&self, index: u8) -> Option<&dyn UsbInterface>;
    fn get_mut_interface(&mut self, index: u8) -> Option<&mut dyn UsbInterface>;
    fn get_hid_interface(&self, index: u8) -> Option<&UsbHID>;
}

pub trait UsbDeviceExtendedRequest {
    fn set_protocol(&mut self, request: u8, interface: u16);
}

pub trait UsbDevice: UsbDeviceStandardRequest + UsbDeviceExtendedRequest {
    fn await_interrupt(&self);

    fn detach(&mut self);

    fn get_port(&self) -> u8;
    /* True means that this device can communicate with the host controller*/
    fn get_state(&self) -> UsbDeviceState;
    fn endpoint_count(&self) -> u8;
    fn device_address(&self) -> u8;
    fn request_packet_address<'a>(&'a self) -> &'a mut UsbStandardDeviceRequest;

    fn get_configuration_count(&self) -> u8;
    fn get_configuration(&self, config: u8) -> Option<&dyn UsbConfiguration>;

    fn get_class_code(&self) -> u8;
    fn get_sub_class_code(&self) -> u8;
}

pub type UsbInterruptPollerCallbackFn = fn(&dyn UsbController);

pub trait UsbController {
    fn identity(&self) -> UsbControllerType;
    fn initialize_controller(
        &mut self,
        pci_bus: &PciBus,
        allocator: &mut Allocator,
        isr_vector: u8,
        pci_device: u64,
    ) -> bool;

    fn gather_device_information(&mut self) -> bool;
    fn configure_devices(&mut self) -> bool;

    fn start(&mut self);
    fn stop(&mut self);
    fn error_present(&self) -> bool;
    fn number_of_active_devices(&self) -> u16;
    fn number_of_potential_devices(&self) -> u16;

    fn untraited_work0(&mut self) -> Option<bool>; // after initialize_controller
    fn untraited_work1(&mut self) -> Option<bool>; // after gather_device_information
    fn untraited_work2(&mut self) -> Option<bool>; // after configure_devices

    fn get_device(&self, index: u16) -> Option<&dyn UsbDevice>;
    fn get_mut_device(&mut self, index: u16) -> Option<&mut dyn UsbDevice>;
    /**
     *
     */
    fn install_interrupt_poller(
        &mut self,
        device: &dyn UsbDevice,
        endpoint: &dyn UsbEndpoint,
        frame: u8,
        report_address: u32,
        bytes_to_transfer: u16,
        callback: Option<UsbInterruptPollerCallbackFn>,
    );
}
