use core::{cmp::max, ffi::c_void, ptr::null_mut, slice};

use crate::{
    drivers::usb::{
        ehci::data_structures::QueueHead,
        independent::{Direction, UsbTransferType},
        ohci::structures::endpoint::EndpointDescriptorBitPart::S,
        standard_requests::{
            UsbDeviceStandardRequest, UsbEndpointDescriptor, UsbStandardDeviceRequest,
        },
        traits::UsbEndpoint,
        uhci::{
            UHCI_CONTROLLER, Uhci,
            data_structures::{
                RawUhciTransferDescriptor, UhciQueueHead, UhciTransferDescriptor,
                UhciTransferDescriptorBitPart, UhciTransferDescriptorPart,
            },
            structures::{
                QUEUE_HEAD_CONTROL_SIMPLE, QUEUE_HEAD_WAS_CONTROL, QUEUE_HEAD_WAS_CUSTOM, device,
                frame_list::UhciFrameList,
            },
        },
    },
    hal::print::simple_kernel_panic,
};
pub struct UhciControlEndpoint {
    endpoint_number: u8,
    maximum_packet_size: u16,
    transfer_descriptors: *mut c_void,
    num_transfer_descriptors: u8,
    pub(in crate::drivers::usb::uhci) queue_head: UhciQueueHead,
    pub(in crate::drivers::usb::uhci) transfer_descriptor_base_offset: u8,
    interval: u16,
}

impl UhciControlEndpoint {
    pub const fn empty() -> Self {
        return Self {
            endpoint_number: 0,
            maximum_packet_size: 0,
            transfer_descriptors: null_mut(),
            queue_head: UhciQueueHead::new(null_mut()),
            transfer_descriptor_base_offset: 0,
            num_transfer_descriptors: 0,
            interval: 0,
        };
    }

    pub fn default_control() -> Self {
        return Self {
            endpoint_number: 0,
            maximum_packet_size: 8,
            transfer_descriptors: null_mut(),
            queue_head: UhciQueueHead::new(null_mut()),
            transfer_descriptor_base_offset: 0,
            num_transfer_descriptors: 0,
            interval: UhciFrameList::CONTROL_ENDPOINT_CALL_TIME as u16,
        };
    }

    fn send_over_sized_control_read(
        &mut self,
        request: *const UsbStandardDeviceRequest,
        mut dst_buffer: u32,
        buffer_length: u16,
        ioc: bool,
    ) {
        #[allow(static_mut_refs)]
        let custom_tds_mb = unsafe {
            UHCI_CONTROLLER
                .private_physical_memory
                .alloc_zero(1)
                .unwrap()
        };

        let device_address;
        let endpoint_number;
        {
            let td = UhciTransferDescriptor::new(self.transfer_descriptors as *mut u32);
            device_address = td.get_part(UhciTransferDescriptorPart::DeviceAddress);
            endpoint_number = td.get_part(UhciTransferDescriptorPart::EndPt);
        }

        #[allow(static_mut_refs)]
        let page_offset = (custom_tds_mb.base
            - unsafe { UHCI_CONTROLLER.private_physical_memory.lowest_address() })
            / 0x1000;
        let td_ptrs = custom_tds_mb.as_mut_ptr::<RawUhciTransferDescriptor>();
        let mut in_tds_required = buffer_length / self.maximum_packet_size;
        if buffer_length % self.maximum_packet_size != 0 {
            in_tds_required += 1;
        }

        let mut setup_td = UhciTransferDescriptor::from_raw(td_ptrs);
        setup_td.refresh_for_setup(request);
        setup_td.link_next_td(unsafe { td_ptrs.add(1) } as u32, true);
        setup_td.set_part(UhciTransferDescriptorPart::DeviceAddress, device_address);
        setup_td.set_part(UhciTransferDescriptorPart::EndPt, endpoint_number);

        let in_tds = unsafe { slice::from_raw_parts_mut(td_ptrs.add(1), in_tds_required as usize) };
        let mut buf_rem = buffer_length;
        let mut toggle = true;
        for in_td in &mut *in_tds {
            let mut td = UhciTransferDescriptor::from_raw(unsafe { &raw mut (*in_td) });
            let took;
            if self.maximum_packet_size >= buf_rem {
                took = buf_rem;
            } else {
                took = self.maximum_packet_size;
            }
            td.refresh_for_in(took - 1, false, toggle, true);
            td.write_buffer_pointer(dst_buffer);
            td.link_next_td(unsafe { (&raw const (*in_td)).add(1) } as u32, true);
            td.set_part(UhciTransferDescriptorPart::DeviceAddress, device_address);
            td.set_part(UhciTransferDescriptorPart::EndPt, endpoint_number);
            buf_rem -= took;
            dst_buffer += took as u32;
            toggle = !toggle;
        }
        let mut status_td =
            UhciTransferDescriptor::from_raw(unsafe { td_ptrs.add(1 + in_tds_required as usize) });
        status_td.refresh_for_status(false);
        status_td.set(UhciTransferDescriptorBitPart::T, false);
        status_td.write_link_pointer(
            QUEUE_HEAD_WAS_CUSTOM
                | (self.transfer_descriptor_base_offset as u32) << 16
                | (page_offset as u32) << 8,
        );
        status_td.set_part(UhciTransferDescriptorPart::DeviceAddress, device_address);
        status_td.set_part(UhciTransferDescriptorPart::EndPt, endpoint_number);
        setup_td.set_part(UhciTransferDescriptorPart::Status, 1 << 7); // Activates
        self.queue_head
            .set_queue_element_link_pointer(setup_td.address());
        self.queue_head.set_queue_element_link_t(false);
    }
    fn send_right_sized_control_read(
        &mut self,
        request: *const UsbStandardDeviceRequest,
        dst_buffer: u32,
        dst_length: u16,
        ioc: bool,
    ) {
        let mut setup_td = UhciTransferDescriptor::new(self.transfer_descriptors as *mut u32);
        let mut in_td = UhciTransferDescriptor::new(unsafe {
            self.transfer_descriptors
                .add(size_of::<RawUhciTransferDescriptor>())
        } as *mut u32);
        let mut status_td = UhciTransferDescriptor::new(unsafe {
            self.transfer_descriptors
                .add(size_of::<RawUhciTransferDescriptor>() * 2)
        } as *mut u32);

        setup_td.refresh_for_setup(request);
        if dst_length == 0 {
            in_td.refresh_for_in(0x7FF, false, true, true);
        } else {
            in_td.refresh_for_in(dst_length - 1, false, true, true);
        }
        status_td.refresh_for_status(false);

        setup_td.link_next_td(in_td.address(), true);
        in_td.write_buffer_pointer(dst_buffer);
        in_td.link_next_td(status_td.address(), true);

        status_td.set(UhciTransferDescriptorBitPart::T, true);
        status_td.write_buffer_pointer(0);
        status_td.write_link_pointer(
            QUEUE_HEAD_WAS_CONTROL | (self.transfer_descriptor_base_offset as u32) << 8,
        );

        setup_td.set_part(UhciTransferDescriptorPart::Status, 1 << 7); // Activates
        self.queue_head.set_queue_element_link_t(false);
    }

    pub fn send_control_read(
        &mut self,
        request: *const UsbStandardDeviceRequest,
        dst_buffer: u32,
        buffer_length: u16,
        ioc: bool,
    ) {
        if buffer_length > self.maximum_packet_size {
            self.send_over_sized_control_read(request, dst_buffer, buffer_length, ioc);
        } else {
            self.send_right_sized_control_read(request, dst_buffer, buffer_length, ioc);
        }
    }

    pub fn send_control_without_data(
        &mut self,
        request: *const UsbStandardDeviceRequest,
        ioc: bool,
    ) {
        let mut setup_td = UhciTransferDescriptor::new(self.transfer_descriptors as *mut u32);
        let mut status_td = UhciTransferDescriptor::new(unsafe {
            self.transfer_descriptors
                .add(size_of::<RawUhciTransferDescriptor>())
        } as *mut u32);

        setup_td.refresh_for_setup(request);
        status_td.refresh_for_status(true); // YUP, THIS IS CORRECT

        setup_td.link_next_td(status_td.address(), true);

        status_td.set(UhciTransferDescriptorBitPart::T, true);
        status_td.write_buffer_pointer(0);
        status_td.write_link_pointer(
            QUEUE_HEAD_WAS_CONTROL
                | QUEUE_HEAD_CONTROL_SIMPLE
                | (self.transfer_descriptor_base_offset as u32) << 8,
        );

        setup_td.set_part(UhciTransferDescriptorPart::Status, 1 << 7); // Activates
        self.queue_head.set_queue_element_link_t(false);
    }

    pub fn send_right_sized_control_write(
        &mut self,
        request: *const UsbStandardDeviceRequest,
        dst_buffer: u32,
        out_length: u16,
        ioc: bool,
    ) {
        let mut setup_td = UhciTransferDescriptor::new(self.transfer_descriptors as *mut u32);
        let mut out_td = UhciTransferDescriptor::new(unsafe {
            self.transfer_descriptors
                .add(size_of::<RawUhciTransferDescriptor>())
        } as *mut u32);
        let mut status_td = UhciTransferDescriptor::new(unsafe {
            self.transfer_descriptors
                .add(size_of::<RawUhciTransferDescriptor>() * 2)
        } as *mut u32);

        setup_td.refresh_for_setup(request);
        if out_length == 0 {
            out_td.refresh_for_out(0x7FF, false, true, true);
        } else {
            out_td.refresh_for_out(out_length - 1, false, true, true);
            out_td.write_buffer_pointer(dst_buffer);
        }

        status_td.refresh_for_status(true);
        status_td.write_buffer_pointer(0);

        setup_td.link_next_td(out_td.address(), true);
        out_td.link_next_td(status_td.address(), true);

        status_td.set(UhciTransferDescriptorBitPart::T, true);
        status_td.write_link_pointer(
            QUEUE_HEAD_WAS_CONTROL | (self.transfer_descriptor_base_offset as u32) << 8,
        );

        setup_td.set_part(UhciTransferDescriptorPart::Status, 1 << 7); // Activates
        self.queue_head.set_queue_element_link_t(false);
        todo!("Test send_right_sized_control_write in Uhci\n");
    }

    pub fn send_over_sized_control_write(
        &mut self,
        request: *const UsbStandardDeviceRequest,
        mut dst_buffer: u32,
        buffer_length: u16,
        ioc: bool,
    ) {
        #[allow(static_mut_refs)]
        let custom_tds_mb = unsafe {
            UHCI_CONTROLLER
                .private_physical_memory
                .alloc_zero(1)
                .unwrap()
        };

        let device_address;
        let endpoint_number;
        {
            let td = UhciTransferDescriptor::new(self.transfer_descriptors as *mut u32);
            device_address = td.get_part(UhciTransferDescriptorPart::DeviceAddress);
            endpoint_number = td.get_part(UhciTransferDescriptorPart::EndPt);
        }

        #[allow(static_mut_refs)]
        let page_offset = (custom_tds_mb.base
            - unsafe { UHCI_CONTROLLER.private_physical_memory.lowest_address() })
            / 0x1000;
        let td_ptrs = custom_tds_mb.as_mut_ptr::<RawUhciTransferDescriptor>();
        let mut in_tds_required = buffer_length / self.maximum_packet_size;
        if buffer_length % self.maximum_packet_size != 0 {
            in_tds_required += 1;
        }

        let mut setup_td = UhciTransferDescriptor::from_raw(td_ptrs);
        setup_td.refresh_for_setup(request);
        setup_td.link_next_td(unsafe { td_ptrs.add(1) } as u32, true);
        setup_td.set_part(UhciTransferDescriptorPart::DeviceAddress, device_address);
        setup_td.set_part(UhciTransferDescriptorPart::EndPt, endpoint_number);

        let in_tds = unsafe { slice::from_raw_parts_mut(td_ptrs.add(1), in_tds_required as usize) };
        let mut buf_rem = buffer_length;
        let mut toggle = true;
        for in_td in &mut *in_tds {
            let mut td = UhciTransferDescriptor::from_raw(unsafe { &raw mut (*in_td) });
            let took;
            if self.maximum_packet_size >= buf_rem {
                took = buf_rem;
            } else {
                took = self.maximum_packet_size;
            }
            td.refresh_for_out(took - 1, false, toggle, true);
            td.write_buffer_pointer(dst_buffer);
            td.link_next_td(unsafe { (&raw const (*in_td)).add(1) } as u32, true);
            td.set_part(UhciTransferDescriptorPart::DeviceAddress, device_address);
            td.set_part(UhciTransferDescriptorPart::EndPt, endpoint_number);
            buf_rem -= took;
            dst_buffer += took as u32;
            toggle = !toggle;
        }
        let mut status_td =
            UhciTransferDescriptor::from_raw(unsafe { td_ptrs.add(1 + in_tds_required as usize) });
        status_td.refresh_for_status(true);
        status_td.set(UhciTransferDescriptorBitPart::T, false);
        status_td.write_link_pointer(
            QUEUE_HEAD_WAS_CUSTOM
                | (self.transfer_descriptor_base_offset as u32) << 16
                | (page_offset as u32) << 8,
        );
        status_td.set_part(UhciTransferDescriptorPart::DeviceAddress, device_address);
        status_td.set_part(UhciTransferDescriptorPart::EndPt, endpoint_number);
        setup_td.set_part(UhciTransferDescriptorPart::Status, 1 << 7); // Activates
        self.queue_head
            .set_queue_element_link_pointer(setup_td.address());
        self.queue_head.set_queue_element_link_t(false);
    }
    pub fn send_control_write(
        &mut self,
        request: *const UsbStandardDeviceRequest,
        dst_buffer: u32,
        dst_length: u16,
        ioc: bool,
    ) {
        if dst_length > self.maximum_packet_size {
            self.send_over_sized_control_write(request, dst_buffer, dst_length, ioc);
        } else {
            self.send_right_sized_control_write(request, dst_buffer, dst_length, ioc);
        }
    }
}

impl UsbEndpoint for UhciControlEndpoint {
    fn endpoint_number(&self) -> u8 {
        return self.endpoint_number;
    }
    fn get_direction(&self) -> Direction {
        return Direction::Invalid;
    }
    fn get_interval_in_ms(&self) -> u16 {
        return self.interval;
    }
    fn get_maximum_packet_size(&self) -> u16 {
        return self.maximum_packet_size;
    }
    fn get_transfer_type(&self) -> UsbTransferType {
        return UsbTransferType::Control;
    }
    fn set_address_and_length(&mut self, base: *mut core::ffi::c_void, transfer_descriptors: u8) {
        self.num_transfer_descriptors = transfer_descriptors;
        self.transfer_descriptors = base;
    }
    fn update_max_packet_size(&mut self, max_packet_size: u16) {
        self.maximum_packet_size = max_packet_size;
    }
}
/* Isochronous/Interrupt or Bulk*/
pub struct UhciTransferDescriptorArray {
    transfer_descriptors: &'static [RawUhciTransferDescriptor],
    interval: u16,
    queue_head: UhciQueueHead,
}

impl UhciTransferDescriptorArray {
    pub fn new(
        transfer_descriptors: &'static [RawUhciTransferDescriptor],
        interval: u16,
        queue_head: UhciQueueHead,
    ) -> Self {
        return Self {
            transfer_descriptors,
            interval,
            queue_head,
        };
    }
    #[inline(always)]
    pub fn activate(&mut self) {
        self.queue_head.set_queue_head_link_t(false);
    }
    pub fn queue_head(&self) -> UhciQueueHead {
        return self.queue_head;
    }
    #[inline(always)]
    pub fn activate_ioc(&self) {
        self.transfer_descriptors[self.transfer_descriptors.len() - 1]
            .wrapped()
            .set(UhciTransferDescriptorBitPart::Ioc, true);
    }

    pub fn set_buffer(&self, mut buffer: u32) {
        for raw_td in &*self.transfer_descriptors {
            raw_td.wrapped().write_buffer_pointer(buffer);
            buffer += raw_td
                .wrapped()
                .get_part(UhciTransferDescriptorPart::MaxLen)
                + 1;
        }
    }

    pub fn initialize(
        &mut self,
        transfer_descriptors: &'static [RawUhciTransferDescriptor],
        interval: u16,
        queue_head: UhciQueueHead,
    ) {
        self.interval = interval;
        self.queue_head = queue_head;
        self.transfer_descriptors = transfer_descriptors;
    }
}

pub enum UhciGeneralEndpointRealEndpoint {
    Unassigned,
    Control(&'static mut UhciControlEndpoint), // Control
    TdArray(&'static mut UhciTransferDescriptorArray), // Interrupt or Bulk
    Isochronous(UhciTransferDescriptor),
}

pub struct UhciGeneralEndpoint {
    endpoint_address: u8,
    bm_attributes: u8,
    w_max_packet_size: u16,
    pub(in crate::drivers::usb::uhci) b_interval: u8,
    pub(in crate::drivers::usb::uhci) real_endpoint: UhciGeneralEndpointRealEndpoint,
}

impl UhciGeneralEndpoint {
    pub fn from_raw(r#in: &UsbEndpointDescriptor) -> Self {
        return Self {
            endpoint_address: r#in.b_endpoint_address,
            bm_attributes: r#in.bm_attributes,
            w_max_packet_size: r#in.w_max_packet_size,
            b_interval: r#in.b_interval,
            real_endpoint: UhciGeneralEndpointRealEndpoint::Unassigned,
        };
    }

    pub fn get_endpoint_address(&self) -> u8 {
        return self.endpoint_address;
    }
    pub fn get_bm_attributes(&self) -> u8 {
        return self.bm_attributes;
    }
    pub fn get_max_packet_size(&self) -> u16 {
        return self.w_max_packet_size;
    }
}

impl UsbEndpoint for UhciGeneralEndpoint {
    fn endpoint_number(&self) -> u8 {
        return self.endpoint_address & 0b1111;
    }
    fn get_direction(&self) -> Direction {
        match (self.endpoint_address >> 7) {
            0 => return Direction::Out,
            1 => return Direction::In,
            _ => simple_kernel_panic("UhciGeneralEndpoint/get_direction", "How?\n"),
        }
    }
    fn get_interval_in_ms(&self) -> u16 {
        return self.b_interval as u16;
    }
    fn get_maximum_packet_size(&self) -> u16 {
        return self.w_max_packet_size;
    }
    fn get_transfer_type(&self) -> UsbTransferType {
        return match self.bm_attributes & 0b11 {
            0 => UsbTransferType::Control,
            1 => UsbTransferType::Isochronous,
            2 => UsbTransferType::Bulk,
            3 => UsbTransferType::Interrupt,
            _ => simple_kernel_panic("UhciGeneralEndpoint/get_transfer_type", "How?\n"),
        };
    }
    fn set_address_and_length(&mut self, base: *mut c_void, transfer_descriptors: u8) {
        simple_kernel_panic(
            "UhciGeneralEndpoint/set_address_and_length",
            "Not supported\n",
        )
    }
    fn update_max_packet_size(&mut self, max_packet_size: u16) {
        self.w_max_packet_size = max_packet_size;
    }
}
