use core::ffi::c_void;

use crate::drivers::usb::{
    ehci::structures::device::EhciDevice,
    traits::{UsbDevice, UsbDeviceExtendedRequest, UsbEndpoint},
};

impl UsbDeviceExtendedRequest for EhciDevice {
    fn set_protocol(&mut self, request: u8, interface: u16) {
        let descriptor = self.request_packet_address();
        descriptor.bm_request_type = 0x21;
        descriptor.b_request = request;
        descriptor.w_value = 0;
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
