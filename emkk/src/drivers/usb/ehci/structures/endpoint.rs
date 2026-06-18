use core::{ffi::c_void, ptr::null_mut};

use crate::{
    drivers::usb::{
        ehci::{
            EHCI_CONTROLLER,
            data_structures::{PidCode, QueueElementTransferDescriptor, QueueHead},
        },
        independent::{Direction, UsbEndpointError, UsbTransferType},
        ohci::structures::endpoint::EndpointDescriptorBitPart::S,
        standard_requests::UsbEndpointDescriptor,
        traits::UsbEndpoint,
    },
    hal::print::simple_kernel_panic,
};

pub struct EhciEndpointInformation {
    endpoint_address: u8,
    attributes: u8,
    max_packet_size: u16,
    interval: u8,
}

impl Default for EhciEndpointInformation {
    fn default() -> Self {
        return Self {
            endpoint_address: 0,
            attributes: 0,
            max_packet_size: 0,
            interval: 0,
        };
    }
}

pub struct EhciEndpoint {
    endpoint_index: u8,
    maximum_qtds: u16,
    qtd_base: *mut c_void,
    designated_queue_head: QueueHead,
    setup_qtd_index: u16,
    endpoint_information: EhciEndpointInformation,
}

impl Default for EhciEndpoint {
    fn default() -> Self {
        return Self {
            endpoint_index: 0,
            maximum_qtds: 0,
            qtd_base: null_mut(),
            designated_queue_head: QueueHead::new(0),
            setup_qtd_index: 0,
            endpoint_information: EhciEndpointInformation::default(),
        };
    }
}

impl EhciEndpoint {
    pub fn new(
        endpoint_index: u8,
        base: *mut c_void,
        maximum_qtds: u8,
        designated_queue_head: QueueHead,
    ) -> Self {
        return Self {
            endpoint_index,
            maximum_qtds: maximum_qtds as u16,
            qtd_base: base,
            designated_queue_head,
            setup_qtd_index: 0,
            endpoint_information: EhciEndpointInformation::default(),
        };
    }

    pub fn full_new_from_raw(
        endpoint_descriptor: &UsbEndpointDescriptor,
        maximum_qtds: u16,
        qtd_base: *mut c_void,
        designated_queue_head: QueueHead,
    ) -> Self {
        return Self {
            endpoint_index: endpoint_descriptor.b_endpoint_address & 0xF,
            maximum_qtds,
            qtd_base,
            designated_queue_head,
            setup_qtd_index: 0,
            endpoint_information: EhciEndpointInformation {
                endpoint_address: endpoint_descriptor.b_endpoint_address,
                attributes: endpoint_descriptor.bm_attributes,
                max_packet_size: endpoint_descriptor.w_max_packet_size,
                interval: endpoint_descriptor.b_interval,
            },
        };
    }

    pub fn full_new(
        endpoint_index: u8,
        maximum_qtds: u16,
        qtd_base: *mut c_void,
        designated_queue_head: QueueHead,
        endpoint_address: u8,
        attributes: u8,
        max_packet_size: u16,
        interval: u8,
    ) -> Self {
        return Self {
            endpoint_index,
            maximum_qtds,
            qtd_base,
            designated_queue_head,
            setup_qtd_index: 0,
            endpoint_information: EhciEndpointInformation {
                endpoint_address,
                attributes,
                max_packet_size,
                interval,
            },
        };
    }

    pub fn get_designated_queue_head(&mut self) -> &mut QueueHead {
        return &mut self.designated_queue_head;
    }

    #[inline(always)]
    pub fn get_setup_qtd(&self) -> u64 {
        return unsafe { self.qtd_base.add((self.setup_qtd_index as usize) * 32) } as u64;
    }
    #[inline(always)]
    pub fn get_after_setup(&self, index: u16) -> u64 {
        return unsafe {
            self.qtd_base
                .add((((self.setup_qtd_index + index) % self.maximum_qtds) as usize) * 32)
        } as u64;
    }
    #[inline(always)]
    pub fn advance_setup_qtd(&mut self, chained_qtds: u16) {
        let new_index = (self.setup_qtd_index + chained_qtds + 1) % self.maximum_qtds;
        self.setup_qtd_index = new_index;
    }
}

impl UsbEndpoint for EhciEndpoint {
    fn get_designated_queue_head_address(&self) -> u32 {
        return self.designated_queue_head.get_address() as u32;
    }

    fn get_maximum_packet_size(&self) -> u16 {
        return self.endpoint_information.max_packet_size;
    }

    fn update_max_packet_size(&mut self, max_packet_size: u16) {
        return self.endpoint_information.max_packet_size = max_packet_size;
    }

    fn set_address_and_length(&mut self, base: *mut c_void, maximum_qtds: u8) {
        if base as u64 > 0xFFFFFFFF {
            simple_kernel_panic(
                "EhciEndpoint/set_address_and_length",
                "Address is above 0xFFFFFFFF\n",
            );
        }
        self.qtd_base = base;
        self.maximum_qtds = maximum_qtds as u16;
    }

    fn get_transfer_type(&self) -> UsbTransferType {
        return UsbTransferType::from_u8(self.endpoint_information.attributes & 0b11);
    }
    fn get_direction(&self) -> Direction {
        return Direction::from_bool(1 == ((self.endpoint_information.endpoint_address >> 7) & 1));
    }

    fn calculate_interval_micro_frames(&self) -> u16 {
        return 1 << (self.endpoint_information.interval - 1);
    }

    fn endpoint_number(&self) -> u8 {
        return self.endpoint_index;
    }
    fn control_without_data(
        &mut self,
        qh_address: *mut core::ffi::c_void,
        status_page_buffer0_address: u32,
    ) {
        let mut setup_qtd = QueueElementTransferDescriptor::new(self.get_setup_qtd());
        let mut status_qtd = QueueElementTransferDescriptor::new(self.get_after_setup(1));

        {
            setup_qtd.reset();
            setup_qtd
                .next_qtd_pointer()
                .set_transfer_element_pointer(self.get_after_setup(1) as u32);
            setup_qtd.set_pid_code(PidCode::SetupToken);
            setup_qtd.set_total_bytes_to_transfer(8);
            setup_qtd.set_buffer_pointer0((status_page_buffer0_address >> 12) << 12);
            setup_qtd.set_current_offset((status_page_buffer0_address & 0xFFF) as u16);
            setup_qtd.set_status_bit(7); // Activate
            setup_qtd.next_qtd_pointer().set_terminate(false);
        }

        {
            status_qtd.reset();
            status_qtd.set_interrupt_on_completion(true);
            status_qtd.set_pid_code(PidCode::InToken);
            status_qtd.set_dt(true);
            status_qtd.set_status_bit(7); // Activate
        }
        let mut qh = QueueHead::new(qh_address as u64);
        qh.next_qtd_pointer()
            .set_transfer_element_pointer(self.get_setup_qtd() as u32);
        qh.next_qtd_pointer().set_terminate(false);
        qh.clear_status_bit(7);
        #[allow(static_mut_refs)]
        unsafe {
            EHCI_CONTROLLER
                .qhs_to_disable
                .enqueue(QueueHead::new(qh_address as u64))
        };

        if unsafe { EHCI_CONTROLLER.preinserted_answered } != 0 {
            QueueHead::new(qh_address as u64).clear_status_bit(7);
            #[allow(static_mut_refs)]
            unsafe {
                EHCI_CONTROLLER.preinserted_answered -= 1;
                EHCI_CONTROLLER.qhs_to_disable.dequeue_silent();
            }
        }

        self.advance_setup_qtd(1);
    }
    fn control_with_data(
        &mut self,
        qh_address: *const c_void,
        setup_page_buffer0_address: u32,
        base_ptr: u32,
        mut ptr_length: u16,
        data_in: bool,
    ) -> Option<UsbEndpointError> {
        let mut setup_qtd = QueueElementTransferDescriptor::new(self.get_setup_qtd() as u64);

        let mut data_qtds_needed = ptr_length / 0x5000;
        if ptr_length % 0x5000 != 0 {
            data_qtds_needed += 1;
        }

        if data_qtds_needed + 2 > self.maximum_qtds as u16 {
            return Option::Some(UsbEndpointError::MaximumQTDsExceeded);
        }

        let mut current_base_ptr = base_ptr;
        for d in 0..data_qtds_needed {
            let mut data_qtd = QueueElementTransferDescriptor::new(self.get_after_setup(d + 1));
            let aligned_base_ptr = current_base_ptr & !0xFFF;
            let start_base_ptr = current_base_ptr;
            data_qtd.reset();
            data_qtd.set_current_offset((current_base_ptr & 0xFFF) as u16);
            data_qtd.set_buffer_pointer0(aligned_base_ptr);
            data_qtd.set_buffer_pointer1(aligned_base_ptr + 0x1000);
            data_qtd.set_buffer_pointer2(aligned_base_ptr + 0x2000);
            data_qtd.set_buffer_pointer3(aligned_base_ptr + 0x3000);
            data_qtd.set_buffer_pointer4(aligned_base_ptr + 0x4000);
            current_base_ptr = aligned_base_ptr + 0x4000;
            data_qtd.set_pid_code(PidCode::inout_from_bool(data_in));
            let transfer_length = (current_base_ptr - start_base_ptr) as u16;
            if transfer_length > ptr_length {
                data_qtd.set_total_bytes_to_transfer(ptr_length);
                ptr_length = 0;
            } else {
                data_qtd.set_total_bytes_to_transfer(transfer_length);
                ptr_length -= transfer_length;
            }
            data_qtd
                .next_qtd_pointer()
                .set_transfer_element_pointer(self.get_after_setup(d + 2) as u32);
            data_qtd.set_status_bit(7); // Active
            data_qtd.next_qtd_pointer().set_terminate(false);
            data_qtd.set_dt(true);
        }

        let mut status_qtd =
            QueueElementTransferDescriptor::new(self.get_after_setup(data_qtds_needed + 1));

        setup_qtd.reset();
        setup_qtd.set_total_bytes_to_transfer(8);
        setup_qtd.set_current_offset((setup_page_buffer0_address & 0xFFF) as u16);
        setup_qtd.set_buffer_pointer0(setup_page_buffer0_address & !0xFFF);
        setup_qtd.set_pid_code(PidCode::SetupToken);
        setup_qtd
            .next_qtd_pointer()
            .set_transfer_element_pointer(self.get_after_setup(1) as u32);
        setup_qtd.set_status_bit(7); // Activate
        setup_qtd.next_qtd_pointer().set_terminate(false);

        status_qtd.reset();
        status_qtd.set_interrupt_on_completion(true);
        status_qtd.set_pid_code(PidCode::inout_from_bool(!data_in));
        status_qtd.set_status_bit(7); // Activate
        status_qtd.next_qtd_pointer().set_terminate(true);
        status_qtd.set_dt(true);
        let mut qh = QueueHead::new(qh_address as u64);
        qh.next_qtd_pointer()
            .set_transfer_element_pointer(self.get_setup_qtd() as u32);
        qh.clear_status_bit(7); // Activate
        qh.next_qtd_pointer().set_terminate(false);

        #[allow(static_mut_refs)]
        unsafe {
            EHCI_CONTROLLER.qhs_to_disable.enqueue(qh)
        };

        if unsafe { EHCI_CONTROLLER.preinserted_answered } != 0 {
            QueueHead::new(qh_address as u64).clear_status_bit(7);

            #[allow(static_mut_refs)]
            unsafe {
                EHCI_CONTROLLER.preinserted_answered -= 1;
                EHCI_CONTROLLER.qhs_to_disable.dequeue_silent();
            }
        }

        self.advance_setup_qtd(1 + data_qtds_needed);
        return Option::None;
    }
}
