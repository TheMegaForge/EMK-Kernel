use core::{cmp::max, ffi::c_void, ptr::null_mut};

use crate::{
    drivers::usb::{
        independent::{Direction, USB_MICRO_FRAME_TO_FRAME_CONVERSION_FACTOR, UsbTransferType},
        standard_requests::{self, UsbEndpointDescriptor, UsbStandardDeviceRequest},
        traits::UsbEndpoint,
        xhci::{
            data_structures::XhciTrbId,
            registers::XhciDoorbell,
            structures::{XhciDataStageTrb, XhciLinkTrb, XhciSetupStageTrb, XhciStatusStageTrb},
        },
    },
    hal::print::simple_kernel_panic,
};

struct RequestedTrb<T> {
    trb: *mut T,
    cycle: bool,
}

pub struct XhciEndpoint {
    pub(in crate::drivers::usb::xhci) enqueue_pointer: *mut u32,
    max_packet_size: u16,
    endpoint_number: u8,
    direction: Direction,
    transfer_type: UsbTransferType,
    doorbell: XhciDoorbell,
    pcs: bool,
}

impl XhciEndpoint {
    pub fn new(
        enqueue_pointer: *mut u32,
        max_packet_size: u16,
        endpoint_number: u8,
        direction: Direction,
        transfer_type: UsbTransferType,
        doorbell: XhciDoorbell,
    ) -> Self {
        return Self {
            enqueue_pointer,
            max_packet_size,
            endpoint_number,
            direction,
            transfer_type,
            doorbell,
            pcs: true,
        };
    }
    /** returns the next free trb and it´s expected cycle state*/
    fn request_trb<T>(&mut self) -> RequestedTrb<T> {
        let r#type = ((unsafe { self.enqueue_pointer.add(3).read_volatile() }) >> 10) & 0x3F;
        if r#type == XhciTrbId::Link as u32 {
            let mut link = XhciLinkTrb::from(self.enqueue_pointer);
            link.set(super::XhciLinkTrbBitPart::C, self.pcs);
            self.enqueue_pointer = link.ring_segment_pointer() as *mut u32;
            if link.is_set(super::XhciLinkTrbBitPart::Tc) {
                self.pcs = !self.pcs;
            }
        }
        let enqueue_ptr = self.enqueue_pointer;
        self.enqueue_pointer = unsafe { self.enqueue_pointer.add(4) };
        return RequestedTrb {
            trb: enqueue_ptr as *mut T,
            cycle: self.pcs,
        };
    }

    pub(in crate::drivers::usb::xhci) fn ring(&self) {
        if self.endpoint_number == 0 {
            self.doorbell.ring(1, 0);
        } else {
            if let Direction::In = self.direction {
                self.doorbell.ring(self.endpoint_number + 2, 0);
            } else {
                self.doorbell.ring(self.endpoint_number + 1, 0);
            }
        }
    }

    pub fn send_control_no_data(
        &mut self,
        request: &UsbStandardDeviceRequest,
        interrupter_target: u16,
        ioc: bool,
    ) {
        let setup = self.request_trb::<XhciSetupStageTrb>();
        let status = self.request_trb::<XhciStatusStageTrb>();

        {
            let setup_trb = unsafe { &mut *setup.trb };
            setup_trb.b_request = request.b_request;
            setup_trb.bm_request_type = request.bm_request_type;
            setup_trb.w_index = request.w_index;
            setup_trb.w_length = request.w_length;
            setup_trb.set_type();
            setup_trb.set_trb_transfer_length();
            setup_trb.set_trt(super::XhciTransferType::NoDataStage);
            setup_trb.set_ioc(false);
            setup_trb.set_idt(true);
            setup_trb.set_interrupter_target(0);
            setup_trb.set_c(setup.cycle);
        }
        {
            let status_trb = unsafe { &mut *status.trb };
            status_trb.set_type();
            status_trb.set_interrupter_target(interrupter_target);
            status_trb.set(super::XhciStatusStageTrbBitPart::Ioc, ioc);
            status_trb.set(super::XhciStatusStageTrbBitPart::Ch, false);
            status_trb.set(super::XhciStatusStageTrbBitPart::C, status.cycle);
            status_trb.set(super::XhciStatusStageTrbBitPart::Dir, true);
            status_trb.set(super::XhciStatusStageTrbBitPart::Ent, false);
        }
        self.ring();
    }

    pub fn send_control_read(
        &mut self,
        request: &UsbStandardDeviceRequest,
        interrupter_target: u16,
        buffer: u64,
        buffer_len: u32,
        ioc: bool,
    ) {
        let setup = self.request_trb::<XhciSetupStageTrb>();
        let data = self.request_trb::<XhciDataStageTrb>();
        let status = self.request_trb::<XhciStatusStageTrb>();
        let mut setup_trb = unsafe { &mut *setup.trb };
        {
            setup_trb.w_value = request.w_value;
            setup_trb.b_request = request.b_request;
            setup_trb.bm_request_type = request.bm_request_type;
            setup_trb.w_length = request.w_length;
            setup_trb.w_index = request.w_index;
            setup_trb.set_type();
            setup_trb.set_trb_transfer_length();
            setup_trb.set_interrupter_target(0);
            setup_trb.set_idt(true);
            setup_trb.set_ioc(false);
            setup_trb.set_trt(super::XhciTransferType::InDataStage);
        }
        {
            let mut data_trb = unsafe { &mut *data.trb };
            data_trb.set_type();
            data_trb.data_buffer = buffer;
            data_trb.trb_transfer_length(buffer_len);
            data_trb.set_interrupter_target(0);
            data_trb.set(super::XhciDataStageTrbBitPart::Dir, true);
            data_trb.set(super::XhciDataStageTrbBitPart::Idt, false);
            data_trb.set(super::XhciDataStageTrbBitPart::Ioc, false);
            data_trb.set(super::XhciDataStageTrbBitPart::Ch, false);
            data_trb.set(super::XhciDataStageTrbBitPart::Ns, false);
            data_trb.set(super::XhciDataStageTrbBitPart::Isp, false);
            data_trb.set(super::XhciDataStageTrbBitPart::Ent, false);
            data_trb.set(super::XhciDataStageTrbBitPart::C, data.cycle);
        }
        {
            let mut status_trb = unsafe { &mut *status.trb };
            status_trb.set_type();
            status_trb.set_interrupter_target(interrupter_target);
            status_trb.set(super::XhciStatusStageTrbBitPart::Dir, false);
            status_trb.set(super::XhciStatusStageTrbBitPart::Ioc, ioc);
            status_trb.set(super::XhciStatusStageTrbBitPart::Ch, false);
            status_trb.set(super::XhciStatusStageTrbBitPart::Ent, false);
            status_trb.set(super::XhciStatusStageTrbBitPart::C, status.cycle);
        }
        setup_trb.set_c(setup.cycle);
        self.ring();
    }
}

impl UsbEndpoint for XhciEndpoint {
    fn endpoint_number(&self) -> u8 {
        return self.endpoint_number;
    }
    fn get_direction(&self) -> crate::drivers::usb::independent::Direction {
        return self.direction;
    }
    fn get_interval_in_ms(&self) -> u16 {
        todo!()
    }
    fn get_maximum_packet_size(&self) -> u16 {
        return self.max_packet_size;
    }
    fn get_transfer_type(&self) -> crate::drivers::usb::independent::UsbTransferType {
        return self.transfer_type;
    }
    fn set_address_and_length(&mut self, base: *mut core::ffi::c_void, transfer_descriptors: u8) {
        _ = base;
        _ = transfer_descriptors;
    }
    fn update_max_packet_size(&mut self, max_packet_size: u16) {
        self.max_packet_size = max_packet_size;
    }
}

pub enum XhciEndpointDescriptorRealEndpoint {
    Unassigned,
    Closed(&'static mut XhciEndpoint),
    Open(&'static mut XhciEndpoint),
}

pub struct XhciEndpointDescriptor {
    bm_attributes: u8,
    endpoint_address: u8,
    pub(in crate::drivers::usb::xhci) interval: u8,
    max_packet_size: u16,
    pub(in crate::drivers::usb::xhci) superspeed_max_burst: u8,
    pub(in crate::drivers::usb::xhci) superspeed_bm_attributes: u8,
    pub(in crate::drivers::usb::xhci) endpoint: XhciEndpointDescriptorRealEndpoint,
}

impl XhciEndpointDescriptor {
    pub fn from_descriptor(endpoint_descriptor: &UsbEndpointDescriptor) -> Self {
        return Self {
            bm_attributes: endpoint_descriptor.bm_attributes,
            endpoint_address: endpoint_descriptor.b_endpoint_address,
            interval: endpoint_descriptor.b_interval,
            max_packet_size: endpoint_descriptor.w_max_packet_size,
            superspeed_max_burst: 0,
            superspeed_bm_attributes: 0,
            endpoint: XhciEndpointDescriptorRealEndpoint::Unassigned,
        };
    }
}

impl UsbEndpoint for XhciEndpointDescriptor {
    fn endpoint_number(&self) -> u8 {
        return self.endpoint_address & 0xF;
    }
    fn get_direction(&self) -> Direction {
        return match self.endpoint_address >> 7 {
            0 => Direction::Out,
            1 => Direction::In,
            _ => simple_kernel_panic("XhciEndpoint/get_direction", "How?\n"),
        };
    }
    fn get_interval_in_ms(&self) -> u16 {
        return 2u16.pow(self.interval as u32 - 1) / USB_MICRO_FRAME_TO_FRAME_CONVERSION_FACTOR;
    }
    fn get_maximum_packet_size(&self) -> u16 {
        return self.max_packet_size;
    }
    fn get_transfer_type(&self) -> UsbTransferType {
        return match self.bm_attributes & 3 {
            0 => UsbTransferType::Control,
            1 => UsbTransferType::Isochronous,
            2 => UsbTransferType::Bulk,
            3 => UsbTransferType::Interrupt,
            _ => simple_kernel_panic("XhciEndpoint/get_transfer_type", "How?\n"),
        };
    }
    fn set_address_and_length(&mut self, base: *mut c_void, transfer_descriptors: u8) {
        simple_kernel_panic("XhciEndpoint/set_address_and_length", "Not supported\n")
    }
    fn update_max_packet_size(&mut self, max_packet_size: u16) {
        simple_kernel_panic("XhciEndpoint/update_max", "Not supported\n")
    }
}
