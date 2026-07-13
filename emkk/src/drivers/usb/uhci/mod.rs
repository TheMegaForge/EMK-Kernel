use core::{arch::asm, ffi::c_void, ptr::null_mut, slice};

use crate::{
    arch::{isr::ISRRegisters, lapic::LocalApic},
    downcast, downcast_mut,
    drivers::usb::{
        ehci::data_structures::QueueHead,
        independent::{
            Direction, UsbControllerType, UsbDescriptorType, UsbDeviceInformation, UsbDeviceState,
            UsbTransferType,
        },
        ohci::structures::{endpoint::EndpointDescriptorBitPart, transfer_descriptors},
        standard_requests::{UsbDeviceStandardRequest, UsbHID},
        traits::{
            UsbConfiguration, UsbController, UsbDevice, UsbEndpoint, UsbInterruptPollerCallbackFn,
        },
        uhci::{
            configuration_parser::UhciDeviceConfiguration,
            data_structures::{
                RawUhciQueueHead, RawUhciTransferDescriptor, UhciBar, UhciPortStatusControlBitPart,
                UhciQueueHead,
                UhciQueuePointer::{self, Queue},
                UhciTransferDescriptor, UhciTransferDescriptorBitPart, UhciTransferDescriptorPart,
                UhciUsbCmdBitPart, UhciUsbInterrupt, UhciUsbStatusBitPart,
            },
            structures::{
                QUEUE_HEAD_CONTROL_SIMPLE, QUEUE_HEAD_WAS_CONTROL, QUEUE_HEAD_WAS_CUSTOM,
                QUEUE_HEAD_WAS_INTERRUPT,
                device::{self, UhciDevice},
                endpoint::{
                    UhciControlEndpoint, UhciGeneralEndpoint, UhciGeneralEndpointRealEndpoint,
                    UhciTransferDescriptorArray,
                },
                frame_list::UhciFrameList,
                interface::UhciInterface,
            },
        },
    },
    fixed_vaddrs::{APPLICATION_CORE_TSS_FIXED_VADDR, ref_processor_mut},
    hal::{
        memory::allocator::{Allocator, MemoryBlock},
        pci_bus::{PciBarIndex, PciBus},
        print::{Module, ModuleWriteMode, simple_kernel_panic},
    },
    info, success,
    time::sleep,
    utils::{
        allocators::PageAllocator,
        memory::{alloc_zero_or_crash, free_or_crash},
        slices::{change_mut_slice_size, change_slice_size, invalid_mut_slice, resize_slice},
        traits::Region,
        u8_rounded_to_highest_power_of_2,
    },
    warn,
};

pub mod configuration_parser;
pub mod data_structures;
pub mod request_impl;
pub mod structures;

pub struct UhciInterfaceData {
    general_endpoints: PageAllocator<UhciGeneralEndpoint>,
    interfaces: PageAllocator<UhciInterface>,
    control_endpoints: &'static mut [UhciControlEndpoint],
    td_arrays: &'static mut [UhciTransferDescriptorArray],

    interrupt_queue_heads_indices: &'static mut [u8],
    interrupt_queue_heads_poller: &'static mut [Option<UsbInterruptPollerCallbackFn>],
    interrupt_queue_heads: &'static mut [RawUhciQueueHead],
    control_queue_heads: &'static mut [RawUhciQueueHead],
    bulk_queue_heads: &'static mut [RawUhciQueueHead],

    bulk_transfer_descriptors: &'static mut [RawUhciTransferDescriptor],
    isochronous_transfer_descriptors: &'static mut [RawUhciTransferDescriptor],
    control_transfer_descriptors: &'static mut [RawUhciTransferDescriptor],
    interrupt_transfer_descriptors: [&'static mut [RawUhciTransferDescriptor]; 11],
    hid_descriptors: &'static mut [UsbHID],

    allocated_control_endpoints: u8,
    allocated_td_arrays: u8,
    allocated_interrupt_queue_heads: u8,
    allocated_control_queue_heads: u8,
    allocated_bulk_queue_heads: u8,
    allocated_bulk_transfer_descriptors: u8,
    allocated_control_transfer_descriptors: u8,
    allocated_isochronous_transfer_descriptors: u16,
    allocated_interrupt_transfer_descriptors: [u8; 11],
}

impl UhciInterfaceData {
    pub const fn empty() -> Self {
        return Self {
            interrupt_queue_heads_indices: invalid_mut_slice(),
            interrupt_queue_heads_poller: invalid_mut_slice(),
            general_endpoints: PageAllocator::empty(),
            interfaces: PageAllocator::empty(),
            control_endpoints: invalid_mut_slice(),
            td_arrays: invalid_mut_slice(),
            bulk_queue_heads: invalid_mut_slice(),
            control_queue_heads: invalid_mut_slice(),
            interrupt_queue_heads: invalid_mut_slice(),
            bulk_transfer_descriptors: invalid_mut_slice(),
            isochronous_transfer_descriptors: invalid_mut_slice(),
            control_transfer_descriptors: invalid_mut_slice(),
            interrupt_transfer_descriptors: [
                invalid_mut_slice(),
                invalid_mut_slice(),
                invalid_mut_slice(),
                invalid_mut_slice(),
                invalid_mut_slice(),
                invalid_mut_slice(),
                invalid_mut_slice(),
                invalid_mut_slice(),
                invalid_mut_slice(),
                invalid_mut_slice(),
                invalid_mut_slice(),
            ],
            hid_descriptors: invalid_mut_slice(),
            allocated_control_endpoints: 0,
            allocated_td_arrays: 0,
            allocated_bulk_queue_heads: 0,
            allocated_control_queue_heads: 0,
            allocated_interrupt_queue_heads: 0,
            allocated_bulk_transfer_descriptors: 0,
            allocated_control_transfer_descriptors: 0,
            allocated_isochronous_transfer_descriptors: 0,
            allocated_interrupt_transfer_descriptors: [0; 11],
        };
    }
    #[inline(always)]
    pub fn static_hid_descriptors(&mut self, index: usize) -> &'static mut [UsbHID] {
        let tmp = self.hid_descriptors.split_at_mut(index).1;
        unsafe { slice::from_raw_parts_mut(tmp.as_mut_ptr(), tmp.len()) }
    }
    pub fn td_arrays_after_inclusive(
        &mut self,
        index: usize,
    ) -> &'static mut [UhciTransferDescriptorArray] {
        let tmp = self.td_arrays.split_at_mut(index).1;
        unsafe { slice::from_raw_parts_mut(tmp.as_mut_ptr(), tmp.len()) }
    }
    pub fn acquire_default_control_endpoint(&mut self) -> &'static mut UhciControlEndpoint {
        let ep = &mut self.control_endpoints[self.allocated_control_endpoints as usize];
        self.allocated_control_endpoints += 1;
        *ep = UhciControlEndpoint::default_control();
        return unsafe { &mut *(&raw mut *ep) };
    }

    pub fn acquire_control_tds(&mut self) -> &'static mut [RawUhciTransferDescriptor] {
        let tds = &mut self.control_transfer_descriptors[self.allocated_control_transfer_descriptors
            as usize
            ..self.allocated_control_transfer_descriptors as usize + 4];
        self.allocated_control_transfer_descriptors += 4;

        return unsafe { slice::from_raw_parts_mut(tds.as_mut_ptr(), tds.len()) };
    }
    #[inline(always)]
    pub fn acquire_control_qh(&mut self) -> UhciQueueHead {
        let qh = self.control_queue_heads[self.allocated_control_queue_heads as usize].wrapped();
        self.allocated_control_queue_heads += 1;
        return qh;
    }
    pub fn acquire_and_initialize_td_array(
        &mut self,
        transfer_descriptors: &[RawUhciTransferDescriptor],
        interval: u16,
        qh: UhciQueueHead,
    ) -> &'static mut UhciTransferDescriptorArray {
        let mut td_array =
            unsafe { &mut *(&raw mut self.td_arrays[self.allocated_td_arrays as usize]) };
        td_array.initialize(
            unsafe { &*(&raw const *transfer_descriptors) },
            interval,
            qh,
        );
        self.allocated_td_arrays += 1;
        return td_array;
    }
    pub fn acquire_bulk_qh(&mut self) -> UhciQueueHead {
        let qh = self.bulk_queue_heads[self.allocated_bulk_queue_heads as usize].wrapped();
        self.allocated_bulk_queue_heads += 1;
        return qh;
    }
    pub fn acquire_bulk_tds(
        &mut self,
        module: &mut Module<'static>,
        tds_needed: u8,
    ) -> &'static [RawUhciTransferDescriptor] {
        if self.allocated_bulk_transfer_descriptors as usize + tds_needed as usize
            > self.bulk_transfer_descriptors.len()
        {
            simple_kernel_panic(
                module.name(),
                "Could not acquire transfer descriptors for bulk endpoint. All transfer descriptors used\n",
            );
        }

        let tds = &self.bulk_transfer_descriptors[self.allocated_bulk_transfer_descriptors as usize
            ..self.allocated_bulk_transfer_descriptors as usize + tds_needed as usize];
        self.allocated_bulk_transfer_descriptors += tds_needed;
        return unsafe { slice::from_raw_parts(tds.as_ptr(), tds.len()) };
    }
    pub fn acquire_qh_and_tds_for_interrupt(
        &mut self,
        module: &mut Module<'static>,
        tds_needed: u8,
        index: usize,
    ) -> (UhciQueueHead, &'static [RawUhciTransferDescriptor]) {
        let qh =
            self.interrupt_queue_heads[self.allocated_interrupt_queue_heads as usize].wrapped();

        if self.allocated_interrupt_transfer_descriptors[index] as usize + tds_needed as usize
            > self.interrupt_transfer_descriptors[index].len()
        {
            simple_kernel_panic(
                module.name(),
                "Could not acquire qh and transfer descriptors for interrupt Endpoint. All transfer descriptors used\n",
            );
        }

        self.interrupt_queue_heads_indices[self.allocated_bulk_queue_heads as usize] = index as u8;

        let transfer_descriptors = &self.interrupt_transfer_descriptors[index][self
            .allocated_interrupt_transfer_descriptors[index]
            as usize
            ..self.allocated_interrupt_transfer_descriptors[index] as usize + tds_needed as usize];

        self.allocated_interrupt_queue_heads += 1;
        self.allocated_interrupt_transfer_descriptors[index] += tds_needed;

        return (qh, unsafe {
            slice::from_raw_parts(transfer_descriptors.as_ptr(), transfer_descriptors.len())
        });
    }
    pub fn initialize(&mut self, module: &mut Module<'static>, allocator: &mut Allocator) {
        self.general_endpoints = PageAllocator::new(allocator, 256);
        self.interfaces = PageAllocator::new(allocator, 128);

        let control_endpoints_mb = alloc_zero_or_crash(
            allocator,
            1,
            module,
            "Could not allocate memory for singular endpoint Array\n",
        );

        let interrupt_indices_mb = alloc_zero_or_crash(
            allocator,
            1,
            module,
            "Could not allocate interrupt qh indices\n",
        );
        let interrupt_pollers_mb = alloc_zero_or_crash(
            allocator,
            2,
            module,
            "Could not allocate interrupt poller Array\n",
        );
        let td_array_mb = alloc_zero_or_crash(
            allocator,
            1,
            module,
            "Could not allocate mmeory for plural endpoint Array\n",
        );
        let queue_heads_mb = alloc_zero_or_crash(
            allocator,
            1,
            module,
            "Could not allocate memory for queue head Array\n",
        );
        let hid_descriptors_mb = alloc_zero_or_crash(
            allocator,
            1,
            module,
            "Could not allocate memory for HID descriptor Array\n",
        );
        /*
         * 4096/16 => 256 Tds
         */
        let non_interrupt_td_mb = alloc_zero_or_crash(
            allocator,
            1,
            module,
            "Could not allocate memory for non-Interrupt transfer descriptors\n",
        );
        let isochronous_td_mb = alloc_zero_or_crash(
            allocator,
            2,
            module,
            "Could not allocate memory for isochronous transfer descriptors\n",
        );
        self.control_endpoints = control_endpoints_mb.as_mut_slice(0);
        self.isochronous_transfer_descriptors = isochronous_td_mb.as_mut_slice(0);
        self.bulk_transfer_descriptors = non_interrupt_td_mb.as_mut_slice_limited(0, 128).unwrap();
        self.control_transfer_descriptors =
            non_interrupt_td_mb.as_mut_slice_limited(128, 128).unwrap();
        self.td_arrays = td_array_mb.as_mut_slice(0);
        self.interrupt_queue_heads = queue_heads_mb.as_mut_slice_limited(0, 11).unwrap();
        self.bulk_queue_heads = queue_heads_mb.as_mut_slice_limited(11, 11).unwrap();
        self.control_queue_heads = queue_heads_mb.as_mut_slice_limited(22, 234).unwrap();
        self.hid_descriptors = hid_descriptors_mb.as_mut_slice(0);
        let interrupt_td_mb = alloc_zero_or_crash(
            allocator,
            1,
            module,
            "Could not allocate memory for interrupt transfer descriptors\n",
        );

        self.interrupt_queue_heads_indices = interrupt_indices_mb
            .as_mut_slice_limited(0, self.interrupt_queue_heads.len() as u64)
            .unwrap();
        self.interrupt_queue_heads_poller = interrupt_pollers_mb
            .as_mut_slice_limited(0, self.interrupt_queue_heads.len() as u64)
            .unwrap();

        self.interrupt_queue_heads_poller
            .iter_mut()
            .for_each(|poller| *poller = Option::None);

        self.interrupt_transfer_descriptors = [
            interrupt_td_mb.as_mut_slice_limited(0, 128).unwrap(),
            interrupt_td_mb.as_mut_slice_limited(128, 20).unwrap(),
            interrupt_td_mb.as_mut_slice_limited(148, 20).unwrap(),
            interrupt_td_mb.as_mut_slice_limited(168, 32).unwrap(),
            interrupt_td_mb.as_mut_slice_limited(200, 16).unwrap(),
            interrupt_td_mb.as_mut_slice_limited(216, 16).unwrap(),
            interrupt_td_mb.as_mut_slice_limited(232, 6).unwrap(),
            interrupt_td_mb.as_mut_slice_limited(238, 6).unwrap(),
            interrupt_td_mb.as_mut_slice_limited(244, 6).unwrap(),
            interrupt_td_mb.as_mut_slice_limited(250, 4).unwrap(),
            interrupt_td_mb.as_mut_slice_limited(254, 2).unwrap(),
        ];
    }
}

pub struct Uhci {
    present: bool,
    module: Module<'static>,
    bar: UhciBar,
    pub(in crate::drivers::usb::uhci) private_physical_memory: Allocator,
    interface_data: UhciInterfaceData,
    active_devices: u16,
    potential_devices: u16,
    frame_list: UhciFrameList,
    devices: &'static mut [UhciDevice],
    error_present: bool,
}

pub(in crate::drivers::usb) static mut UHCI_CONTROLLER: Uhci = Uhci::not_present();
impl Uhci {
    pub const fn not_present() -> Self {
        return Self {
            present: false,
            module: Module::new("Uhci"),
            bar: UhciBar::new(0),
            private_physical_memory: Allocator::empty(),
            active_devices: 0,
            potential_devices: 0,
            frame_list: UhciFrameList::empty(),
            devices: invalid_mut_slice(),
            error_present: false,
            interface_data: UhciInterfaceData::empty(),
        };
    }

    pub fn initialize(&mut self) {
        self.bar.usbcmd().set(UhciUsbCmdBitPart::HcReset, true);
        while self.bar.usbcmd().is_set(UhciUsbCmdBitPart::HcReset) {}
        sleep(20);
        /* Sets MaxPacket to 64 bytes*/
        self.bar.usbcmd().set(UhciUsbCmdBitPart::MaxP, true);
        self.bar
            .usbinterruptenable()
            .set(UhciUsbInterrupt::Timeout, true);
        self.bar
            .usbinterruptenable()
            .set(UhciUsbInterrupt::InterruptOnCompletion, true);
        self.bar
            .usbinterruptenable()
            .set(UhciUsbInterrupt::ShortPacket, true);
        /* Resume interrupt is enabled later*/

        let frame_list_mb = match self.private_physical_memory.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(self.module.name(), "Could not allocate frame list\n"),
        };
        self.bar
            .write_frame_list_base_address(frame_list_mb.base as u32);
        self.frame_list = UhciFrameList::new(
            frame_list_mb.as_mut_ptr(),
            &mut self.private_physical_memory,
            unsafe { &mut *(&raw mut *self.interface_data.interrupt_queue_heads) },
            unsafe { &mut *(&raw mut *self.interface_data.control_queue_heads) },
        );
        self.frame_list.mark_invalid();
        self.start();
        success!(&mut self.module, "Initialized Controller\n");
    }

    pub fn interrupt_check_control(&mut self) {
        for (qh_index, queue_head) in (&mut self.interface_data.control_queue_heads
            [0..self.interface_data.allocated_control_queue_heads as usize])
            .iter_mut()
            .enumerate()
        {
            let mut qh = queue_head.wrapped();
            let next_link = qh.queue_element_link_pointer();
            if next_link & QUEUE_HEAD_WAS_CONTROL != 0 {
                /* Finished */
                let td_base = &raw const self.interface_data.control_transfer_descriptors
                    [(next_link >> 8) as usize & 0xFF];

                let clear_count;
                if next_link & QUEUE_HEAD_CONTROL_SIMPLE != 0 {
                    clear_count = 2;
                } else {
                    clear_count = 3;
                }
                for i in 0..clear_count {
                    let mut td = UhciTransferDescriptor::new(unsafe { td_base.add(i) } as *mut u32);
                    td.set_part(UhciTransferDescriptorPart::CErr, 0b11);
                    td.set_part(UhciTransferDescriptorPart::ActLen, 0);
                    td.set_part(UhciTransferDescriptorPart::MaxLen, 0);
                    td.set_part(UhciTransferDescriptorPart::Status, 0);
                    td.write_buffer_pointer(0);
                }
                let mut end_td = UhciTransferDescriptor::new(
                    unsafe { td_base.add(clear_count - 1) } as *mut u32,
                );
                end_td.write_link_pointer(unsafe { td_base.add(clear_count) } as u32);
                end_td.set(UhciTransferDescriptorBitPart::T, false);

                qh.set_queue_element_link_t(true);
                qh.set_queue_element_link_pointer(td_base as u32);
                let device_address = UhciTransferDescriptor::new(td_base as *mut u32)
                    .get_part(UhciTransferDescriptorPart::DeviceAddress);

                for device in &mut *self.devices {
                    if device.device_address == device_address as u8 {
                        device.signal();
                        break;
                    }
                }
            } else if next_link & QUEUE_HEAD_WAS_CUSTOM != 0 {
                let page_to_free = (next_link >> 8) & 0xFF;
                let original_td_offset = (next_link >> 16) & 0xFF;

                qh.set_queue_element_link_t(true);
                qh.set_queue_element_link_pointer(
                    (&raw const self.interface_data.control_transfer_descriptors
                        [original_td_offset as usize]) as u32,
                );
                let device_address = self.interface_data.control_transfer_descriptors
                    [original_td_offset as usize]
                    .wrapped()
                    .get_part(UhciTransferDescriptorPart::DeviceAddress);

                for device in &mut *self.devices {
                    if device.device_address == device_address as u8 {
                        device.signal();
                        break;
                    }
                }

                let address =
                    self.private_physical_memory.lowest_address() + page_to_free as u64 * 0x1000;
                self.private_physical_memory
                    .free(&MemoryBlock::new(0x1000, address))
                    .unwrap();
            }
        }
    }
    pub fn interrupt_check_interrupt(&mut self) {
        for qh_index in 0..self.interface_data.allocated_interrupt_queue_heads as usize {
            let mut qh = self.interface_data.interrupt_queue_heads[qh_index as usize].wrapped();
            let qele = qh.queue_element_link_pointer();
            if qele & QUEUE_HEAD_WAS_INTERRUPT != 0 {
                let index = self.interface_data.interrupt_queue_heads_indices[qh_index];
                let list = &mut self.interface_data.interrupt_transfer_descriptors[index as usize];

                let td_offset = (qele >> 12) & 0xFFF;
                let num_tds = (qele >> 4) & 0xFF;

                let tds = &mut list[td_offset as usize..td_offset as usize + num_tds as usize];
                let address = tds.as_ptr().addr() as u32;
                let flip_toggle = num_tds % 2 != 0;
                for raw_td in tds {
                    let mut td = raw_td.wrapped();
                    if td.get_part(UhciTransferDescriptorPart::Status) != 0 {
                        simple_kernel_panic(self.module.name(), "Transfer failed\n");
                    }
                    if flip_toggle {
                        td.set(
                            UhciTransferDescriptorBitPart::D,
                            !td.is_set(UhciTransferDescriptorBitPart::D),
                        );
                    }
                    td.set_part(UhciTransferDescriptorPart::ActLen, 0);
                    td.set(UhciTransferDescriptorBitPart::Ioc, true);
                    td.set_part(UhciTransferDescriptorPart::Status, 1 << 7);
                }

                if let Option::Some(callback_fn) =
                    self.interface_data.interrupt_queue_heads_poller[qh_index]
                {
                    (callback_fn)(self);
                }

                qh.set_queue_element_link_pointer(address);
                qh.set_queue_element_link_q(false);
                qh.set_queue_element_link_t(false);
            }
        }
    }
}

fn uhci_interrupt(_: &ISRRegisters) {
    #[allow(static_mut_refs)]
    let uhci_controller = unsafe { &mut UHCI_CONTROLLER };

    let mut status = uhci_controller.bar.usbsts().as_u16();
    status &= !(1 << 5);

    match status {
        1 => {
            uhci_controller.interrupt_check_control();
            uhci_controller.interrupt_check_interrupt();
            uhci_controller
                .bar
                .usbsts()
                .set(UhciUsbStatusBitPart::UsbInt, true); // clears interrupt
        }
        2 => simple_kernel_panic(uhci_controller.module.name(), "Error Happend\n"),
        4 => simple_kernel_panic(uhci_controller.module.name(), "Resume Detected\n"),
        8 => simple_kernel_panic(uhci_controller.module.name(), "Host System Error\n"),
        16 => simple_kernel_panic(
            uhci_controller.module.name(),
            "Host Controller Process Error\n",
        ),
        _ => simple_kernel_panic(uhci_controller.module.name(), "Unknown value\n"),
    }
    LocalApic::from_local_core().send_eoi();
}

fn construct_closed_td_list(
    transfer_descriptors: &[RawUhciTransferDescriptor],
    low_speed: bool,
    vf: bool,
) {
    for (i, raw_td) in (&*transfer_descriptors).iter().enumerate() {
        let mut td = raw_td.wrapped();
        if i != transfer_descriptors.len() - 1 {
            td.write_link_pointer(transfer_descriptors[i as usize + 1].wrapped().address());
        } else {
            td.set(UhciTransferDescriptorBitPart::T, true);
        }
        td.set(UhciTransferDescriptorBitPart::Vf, vf);
        if low_speed {
            td.set(UhciTransferDescriptorBitPart::Ls, true);
        }
        td.set_part(UhciTransferDescriptorPart::CErr, 0b11);
    }
}
/**
 * Activates the TDs
 */
fn configure_transfer_descriptors(
    module: &mut Module<'static>,
    endpoint: &UhciGeneralEndpoint,
    transfer_descriptors: &[RawUhciTransferDescriptor],
    device_address: u8,
    low_speed: bool,
) {
    let mut data_toggle = false;
    let mut length_rem = endpoint.get_max_packet_size();

    for (i, raw_td) in transfer_descriptors.iter().enumerate() {
        let mut td = raw_td.wrapped();
        td.set_part(
            UhciTransferDescriptorPart::DeviceAddress,
            device_address as u32,
        );
        td.set_part(
            UhciTransferDescriptorPart::EndPt,
            endpoint.endpoint_number() as u32,
        );

        match endpoint.get_direction() {
            Direction::In => td.set_part(UhciTransferDescriptorPart::Pid, 0x69),
            Direction::Out => td.set_part(UhciTransferDescriptorPart::Pid, 0xE1),
            _ => simple_kernel_panic(module.name(), "How?\n"),
        }

        assert_ne!(length_rem, 0);
        if length_rem > 64 {
            td.set_part(UhciTransferDescriptorPart::MaxLen, 63);
            length_rem -= 64;
        } else {
            td.set_part(UhciTransferDescriptorPart::MaxLen, length_rem as u32 - 1);
            length_rem = 0;
        }
        td.set(UhciTransferDescriptorBitPart::Ls, low_speed);
        td.set_part(UhciTransferDescriptorPart::Status, 1 << 7);
        td.set_part(UhciTransferDescriptorPart::ActLen, 0);
        td.set(UhciTransferDescriptorBitPart::D, data_toggle);
        data_toggle = !data_toggle;
    }
}

static PORT_CONNECT_MESSAGE: [&'static str; 2] = ["full speed device", "low speed device"];
impl UsbController for Uhci {
    fn configure_devices(&mut self) -> bool {
        let mut current_index = 0;

        for device in &mut *self.devices {
            if let UsbDeviceState::Address = device.state {
                let total_length;
                {
                    let raw_descriptor =
                        device.get_descriptor(UsbDescriptorType::Configuration, 0, Option::None, 9);
                    let configuration_descriptor = raw_descriptor.as_configuration_descriptor();
                    device.set_configuration(configuration_descriptor.b_configuration_value);
                    device.device_information.max_power_ma =
                        configuration_descriptor.b_max_power as u16;
                    device.device_information.num_interfaces =
                        configuration_descriptor.b_num_interfaces;
                    total_length = configuration_descriptor.w_total_length;

                    free_or_crash(
                        &mut self.private_physical_memory,
                        &raw_descriptor.data,
                        &mut self.module,
                        "Could not free configuration descriptor\n",
                    );
                }

                {
                    let raw_descriptor = device.get_descriptor(
                        UsbDescriptorType::Configuration,
                        0,
                        Option::None,
                        total_length,
                    );
                    let configuration_descriptor = raw_descriptor.as_configuration_descriptor();

                    device.configuration = UhciDeviceConfiguration::new(
                        self.interface_data.static_hid_descriptors(current_index),
                        &mut self.interface_data.interfaces,
                        &mut self.interface_data.general_endpoints,
                        unsafe { raw_descriptor.data.as_ptr::<c_void>().add(9) },
                        configuration_descriptor.b_num_interfaces as u16,
                    );
                    current_index += device.configuration.get_hid_interface_count() as usize;
                    free_or_crash(
                        &mut self.private_physical_memory,
                        &raw_descriptor.data,
                        &mut self.module,
                        "Could not free configuration descriptor\n",
                    );
                }

                device.state = UsbDeviceState::Configured
            }
        }
        success!(&mut self.module, "Configured Devices\n");
        return true;
    }
    fn error_present(&self) -> bool {
        return self.error_present;
    }
    fn gather_device_information(&mut self) -> bool {
        for device in &mut *self.devices {
            if let UsbDeviceState::Address = device.state {
                {
                    let raw_device_descriptor =
                        device.get_descriptor(UsbDescriptorType::Device, 0, Option::None, 8);
                    let device_descriptor = raw_device_descriptor.as_device_descriptor();
                    device
                        .control_endpoint
                        .update_max_packet_size(device_descriptor.b_max_packet_size0 as u16);

                    free_or_crash(
                        &mut self.private_physical_memory,
                        &raw_device_descriptor.data,
                        &mut self.module,
                        "Could not free device descriptor\n",
                    );
                }
                {
                    let raw_device_descriptor =
                        device.get_descriptor(UsbDescriptorType::Device, 0, Option::None, 18);
                    let device_descriptor = raw_device_descriptor.as_device_descriptor();

                    device.device_information =
                        UsbDeviceInformation::from_descriptor(device_descriptor);

                    free_or_crash(
                        &mut self.private_physical_memory,
                        &raw_device_descriptor.data,
                        &mut self.module,
                        "Could not free device descriptor\n",
                    );

                    info!(
                        &mut self.module,
                        "Device {} is of class {} and subclass {}\n",
                        device.device_address,
                        device.device_information.device_class,
                        device.device_information.device_sub_class
                    );
                }
            }
        }
        return true;
    }
    fn get_device(&self, index: u16) -> Option<&dyn super::traits::UsbDevice> {
        if index as usize >= self.devices.len() {
            return Option::None;
        }
        return Option::Some(&self.devices[index as usize]);
    }
    fn get_mut_device(&mut self, index: u16) -> Option<&mut dyn super::traits::UsbDevice> {
        if index as usize >= self.devices.len() {
            return Option::None;
        }
        return Option::Some(&mut self.devices[index as usize]);
    }
    fn identity(&self) -> UsbControllerType {
        return UsbControllerType::UHC;
    }
    fn initialize_controller(
        &mut self,
        pci_bus: &PciBus,
        allocator: &mut Allocator,
        isr_vector: u8,
        pci_device: u64,
    ) -> bool {
        let bar = pci_bus.get_bar(pci_device, PciBarIndex::Index4).unwrap();
        self.bar = UhciBar::new(bar.get_address() as u16);
        self.private_physical_memory = allocator.subdivide(64); /* 262.144 bytes*/
        ref_processor_mut().install_isr(uhci_interrupt, isr_vector);
        self.interface_data
            .initialize(&mut self.module, &mut self.private_physical_memory);
        let device_mb = alloc_zero_or_crash(
            &mut self.private_physical_memory,
            1,
            &mut self.module,
            "Could not allocate memory for device Array\n",
        );
        unsafe {
            self.devices = slice::from_raw_parts_mut(device_mb.as_mut_ptr(), 0);
        };
        self.initialize();
        return true;
    }
    fn install_interrupt_poller(
        &mut self,
        device: &mut dyn super::traits::UsbDevice,
        interface_index: u8,
        endpoint_index: u8,
        interval_in_ms: u8,
        report_address: u32,
        bytes_to_transfer: u16,
        callback: Option<super::traits::UsbInterruptPollerCallbackFn>,
    ) {
        let uhci_device = downcast!(device, UsbDevice, UhciDevice);

        if interface_index as usize >= uhci_device.configuration.interfaces.len() {
            simple_kernel_panic(
                self.module.name(),
                "Could not install interrupt poller. Invalid interface\n",
            );
        }
        let interface = &uhci_device.configuration.interfaces[interface_index as usize];
        if endpoint_index as usize >= interface.endpoints.len() {
            simple_kernel_panic(
                self.module.name(),
                "Could not install interrupt poller. Invalid endpoint\n",
            );
        }
        let endpoint = &interface.endpoints[endpoint_index as usize];
        if let UsbTransferType::Interrupt = endpoint.get_transfer_type() {
            let real_endpoint = unsafe {
                &mut *((&raw const endpoint.real_endpoint) as *mut UhciGeneralEndpointRealEndpoint)
            };
            match real_endpoint {
                UhciGeneralEndpointRealEndpoint::TdArray(td_array) => {
                    td_array.activate_ioc();
                    td_array.set_buffer(report_address);
                    let mut qh = td_array.queue_head();

                    if let Option::Some(callback_fn) = callback {
                        let offset = (qh.address()
                            - self.interface_data.interrupt_queue_heads.as_ptr().addr() as u32)
                            / 16;
                        self.interface_data.interrupt_queue_heads_poller[offset as usize] =
                            Option::Some(callback_fn);
                    }

                    qh.set_queue_element_link_t(false);
                }
                _ => simple_kernel_panic(self.module.name(), "How?\n"),
            }
        } else {
            simple_kernel_panic(
                self.module.name(),
                "Could not install interrupt poller. Selected endpoint is not an interrupt endpoint\n",
            )
        }
    }
    fn number_of_active_devices(&self) -> u16 {
        return self.active_devices;
    }
    fn number_of_potential_devices(&self) -> u16 {
        return self.potential_devices;
    }
    fn start(&mut self) {
        self.bar.usbcmd().set(UhciUsbCmdBitPart::Rs, true);
        while self.bar.usbsts().is_set(UhciUsbStatusBitPart::HcHalted) {}
    }
    fn stop(&mut self) {
        self.bar.usbcmd().set(UhciUsbCmdBitPart::Rs, false);
        while !self.bar.usbsts().is_set(UhciUsbStatusBitPart::HcHalted) {}
    }
    fn untraited_work0(&mut self) -> Option<bool> {
        let mut go = true;
        let mut count = 0;
        while go {
            go = self.bar.port(1 + count).is_reserved_1_set();
            if go {
                count += 1;
            }
        }
        self.potential_devices = count as u16;
        resize_slice(&mut self.devices, count as usize);
        let mut current_device_address = 1;

        for device_index in 0usize..self.devices.len() {
            let mut port = self.bar.port(device_index as u8 + 1);
            let device =
                &mut self.devices.split_at_mut(device_index as usize + 1).0[device_index as usize];
            if port.is_set(UhciPortStatusControlBitPart::CurrentConnectStatus) {
                let low_speed = port.is_set(UhciPortStatusControlBitPart::LowSpeedDeviceAttached);
                port.reset();

                *device = UhciDevice::new_addressed(device_index as u8 + 1);
                let control_endpoint = self.interface_data.acquire_default_control_endpoint();

                info!(
                    &mut self.module,
                    "Port {} is connected to a {}\n",
                    device_index + 1,
                    PORT_CONNECT_MESSAGE[low_speed as usize]
                );

                let transfer_descriptors = self.interface_data.acquire_control_tds();

                control_endpoint
                    .set_address_and_length(&raw mut transfer_descriptors[0] as *mut c_void, 4);
                construct_closed_td_list(transfer_descriptors, low_speed, true);

                let mut qh = self.interface_data.acquire_control_qh();
                qh.zero_out();
                qh.set_queue_head_link_t(true);
                qh.set_queue_element_link_pointer(&raw const transfer_descriptors[0] as u32);
                self.frame_list.place_qh(
                    &mut qh,
                    UhciFrameList::CONTROL_ENDPOINT_CALL_TIME,
                    UsbTransferType::Control,
                );
                control_endpoint.transfer_descriptor_base_offset = device_index as u8 * 4;
                control_endpoint.queue_head = qh;
                device.attach_control_endpoint(control_endpoint);
                device.set_address(current_device_address as u16);
                device.device_address = current_device_address;
                transfer_descriptors.iter().for_each(|td| {
                    td.wrapped().set_part(
                        UhciTransferDescriptorPart::DeviceAddress,
                        current_device_address as u32,
                    )
                });
                current_device_address += 1;
                self.active_devices += 1;
            } else {
                *device = UhciDevice::new_detached();
                let control_endpoint = self.interface_data.acquire_default_control_endpoint();
                control_endpoint.transfer_descriptor_base_offset = device_index as u8 * 4;

                let raw_transfer_descriptors = self.interface_data.acquire_control_tds();
                control_endpoint
                    .set_address_and_length(&raw mut raw_transfer_descriptors[0] as *mut c_void, 4);
                construct_closed_td_list(raw_transfer_descriptors, false, true);

                let mut qh = self.interface_data.acquire_control_qh();
                qh.terminate(true, true);
                qh.set_queue_element_link_pointer(&raw const raw_transfer_descriptors[0] as u32);
                self.frame_list.place_qh(
                    &mut qh,
                    UhciFrameList::CONTROL_ENDPOINT_CALL_TIME,
                    UsbTransferType::Control,
                );
                control_endpoint.queue_head = qh;
                device.attach_control_endpoint(control_endpoint);
            }
        }
        Option::Some(true)
    }
    fn untraited_work1(&mut self) -> Option<bool> {
        Option::None
    }
    fn untraited_work2(&mut self) -> Option<bool> {
        let bulk_root: Option<UhciQueueHead>;
        for device in &mut *self.devices {
            if let UsbDeviceState::Detached = device.state {
                continue;
            }

            let low_speed = self
                .bar
                .port(device.get_port())
                .is_set(UhciPortStatusControlBitPart::LowSpeedDeviceAttached);

            if let UsbDeviceState::Configured = device.state {
                let raw_config = device.get_mut_configuration(0).unwrap();
                let config = downcast_mut!(raw_config, UsbConfiguration, UhciDeviceConfiguration);
                for interface in &mut *config.interfaces {
                    for endpoint in &mut *interface.endpoints {
                        match endpoint.get_transfer_type() {
                            UsbTransferType::Control => {
                                warn!(
                                    &mut self.module,
                                    "Device {} has an extra control endpoint. Routing it to the default control endpoint...\n",
                                    device.device_address()
                                );
                                endpoint.real_endpoint =
                                    UhciGeneralEndpointRealEndpoint::Control(unsafe {
                                        &mut *(&raw mut *device.control_endpoint)
                                    });
                            }
                            UsbTransferType::Bulk => {
                                todo!("Test Uhci Bulk Endpoint\n");
                                if let Option::None = bulk_root {
                                    let qh = self.interface_data.acquire_bulk_qh();
                                    qh.terminate(true, true);
                                    bulk_root = Option::Some(qh);
                                }
                                let root = bulk_root.unwrap();
                                let append_qh_to = root.last_head();

                                let qh = self.interface_data.acquire_bulk_qh();
                                unsafe { append_qh_to.address().write_volatile(qh.address() | 2) }
                                let mut tds_needed = endpoint.get_max_packet_size() / 64;
                                if endpoint.get_max_packet_size() % 64 != 0 {
                                    tds_needed += 1;
                                }

                                let transfer_descriptors = self
                                    .interface_data
                                    .acquire_bulk_tds(&mut self.module, tds_needed as u8);

                                construct_closed_td_list(transfer_descriptors, low_speed, true);
                                configure_transfer_descriptors(
                                    &mut self.module,
                                    endpoint,
                                    transfer_descriptors,
                                    device.device_address,
                                    low_speed,
                                );
                                qh.terminate(true, true);
                                qh.set_queue_element_link_pointer(
                                    transfer_descriptors[0].address(),
                                );

                                endpoint.real_endpoint = UhciGeneralEndpointRealEndpoint::TdArray(
                                    self.interface_data.acquire_and_initialize_td_array(
                                        transfer_descriptors,
                                        0xFFFF,
                                        qh,
                                    ),
                                );
                            }
                            UsbTransferType::Isochronous => {
                                todo!("Implement Isochronous\n")
                            }
                            UsbTransferType::Interrupt => {
                                let real_interval =
                                    u8_rounded_to_highest_power_of_2(endpoint.b_interval);
                                let index = real_interval.ilog2();
                                assert!(11 > index);
                                let mut tds_needed = endpoint.get_max_packet_size() / 64;
                                if endpoint.get_max_packet_size() % 64 != 0 {
                                    tds_needed += 1;
                                }
                                let (mut qh, transfer_descriptors) =
                                    self.interface_data.acquire_qh_and_tds_for_interrupt(
                                        &mut self.module,
                                        tds_needed as u8,
                                        index as usize,
                                    );
                                construct_closed_td_list(transfer_descriptors, low_speed, true);
                                configure_transfer_descriptors(
                                    &mut self.module,
                                    endpoint,
                                    transfer_descriptors,
                                    device.device_address,
                                    low_speed,
                                );
                                transfer_descriptors[transfer_descriptors.len() - 1]
                                    .wrapped()
                                    .write_link_pointer(
                                        QUEUE_HEAD_WAS_INTERRUPT
                                            | (unsafe {
                                                transfer_descriptors.as_ptr().offset_from_unsigned(
                                                    self.interface_data
                                                        .interrupt_transfer_descriptors
                                                        [index as usize]
                                                        .as_ptr(),
                                                )
                                            }
                                                as u32
                                                & 0xFFF)
                                                << 12
                                            | (transfer_descriptors.len() as u32 & 0xFF) << 4,
                                    );
                                qh.terminate(true, true);
                                qh.set_queue_element_link_pointer(
                                    transfer_descriptors[0].address(),
                                );
                                self.frame_list.place_qh(
                                    &mut qh,
                                    real_interval as u32,
                                    UsbTransferType::Interrupt,
                                );
                                endpoint.real_endpoint = UhciGeneralEndpointRealEndpoint::TdArray(
                                    self.interface_data.acquire_and_initialize_td_array(
                                        transfer_descriptors,
                                        real_interval as u16,
                                        qh,
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }
        return Option::Some(true);
    }
}

pub fn create_uhci(
    pci_bus: &PciBus,
    pci_device: u64,
    physical_allocator: &mut Allocator,
    isr_vector: u8,
) {
    unsafe {
        let uhci_controller = &raw mut UHCI_CONTROLLER;
        (*uhci_controller).initialize_controller(
            pci_bus,
            physical_allocator,
            isr_vector,
            pci_device,
        );
        (*uhci_controller).untraited_work0();
        (*uhci_controller).gather_device_information();
        (*uhci_controller).configure_devices();
        (*uhci_controller).untraited_work2();
    }
}
