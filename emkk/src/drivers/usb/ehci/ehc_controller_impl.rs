use crate::{
    drivers::usb::{
        ehci::{
            Ehci, EhciInterruptPoller,
            configuration_parser::EhciDeviceConfiguration,
            data_structures::{
                AsynchronousList, EhciCommonType0, EndpointSpeed, Mult, PidCode,
                QueueElementTransferDescriptor, QueueHead,
            },
            registers::{Fladj, LineStatus, UsbBase},
            structures::{endpoint::EhciEndpoint, interface::EhciInterface},
        },
        independent::{CONFIGURATION_DESCRIPTOR_TYPE, DEVICE_DESCRIPTOR_TYPE, UsbControllerType},
        standard_requests::UsbDeviceStandardRequest,
        traits::{
            UsbController, UsbDevice, UsbEndpoint, UsbInterface, UsbInterruptPollerCallbackFn,
        },
    },
    fixed_vaddrs::{EHCI_BAR_FIXED_VADDR, ref_processor_mut},
    hal::{
        memory::allocator::{Allocator, MemoryBlock},
        pci_bus::{PciBarIndex, PciBus},
        print::{Module, simple_kernel_panic},
    },
    info,
    time::sleep,
    utils::{allocators::PageAllocator, queue::Queue, traits::Region},
};

impl UsbController for Ehci {
    fn identity(&self) -> UsbControllerType {
        return UsbControllerType::EHC;
    }
    fn initialize_controller(
        &mut self,
        pci_bus: &PciBus,
        physical_allocator: &mut Allocator,
        isr_vector: u8,
        pci_device: u64,
    ) -> bool {
        self.module = Module::new("Usb/Ehci");
        self.present = true;
        self.isr_vector = isr_vector;
        self.memory_space = physical_allocator.subdivide(32); // 131.072 bytes for eHCI Driver
        self.qhs_to_disable = Queue::new(&mut self.memory_space, 256);
        self.interrupt_pollers_qhs = match self.memory_space.alloc(2) {
            Ok(mb) => {
                if mb.get_base() > 0xFFFFFFFF {
                    simple_kernel_panic(
                        "Ehci/initialize_controller",
                        "Allocated address for interrupt poller Queue Heads is above 0xFFFFFFFF\n",
                    )
                }
                mb.as_mut_ptr()
            }
            Err(_e) => simple_kernel_panic(
                "Ehci/initialize_controller",
                "Could not allocate interrupt poller Queue Heads\n",
            ),
        };
        self.interrupt_pollers_qtds = match self.memory_space.alloc(2) {
            Ok(mb) => {
                if mb.get_base() > 0xFFFFFFFF {
                    simple_kernel_panic(
                        "Ehci/initialize_controller",
                        "Allocated address for interrupt poller Queue Transfer Descriptors is above 0xFFFFFFFF\n",
                    )
                }
                mb.as_mut_ptr()
            }
            Err(_e) => simple_kernel_panic(
                "Ehci/initialize_controller",
                "Could not allocate interrupt poller Queue Transfer Descriptors\n",
            ),
        };
        self.interrupt_pollers = PageAllocator::new(&mut self.memory_space, 170);
        self.interrupt_pollers_designated_qhs = PageAllocator::new(&mut self.memory_space, 128);
        let bar0 = pci_bus.get_bar(pci_device, PciBarIndex::Index0).unwrap();
        if bar0.get_length() / 0x1000 > 4 {
            simple_kernel_panic(
                "UsbController/initialize_controller",
                "more than 16kb required for bar\n",
            );
        }
        bar0.map_to_virtual(
            ref_processor_mut().ref_mut_pager(),
            EHCI_BAR_FIXED_VADDR,
            physical_allocator,
        );
        self.usbbase = UsbBase::new(bar0.get_address(), EHCI_BAR_FIXED_VADDR);

        if self.usbbase.hccparams().eecp() >= 0x40 {
            let eecp = self.usbbase.hccparams().eecp();
            let val = pci_bus.read_configuration_space_u32(pci_device, eecp as u16);
            pci_bus.write_configuration_space_u32(pci_device, eecp as u16, val | 1 << 24);
            sleep(200);
            pci_bus.write_configuration_space_u32(pci_device, eecp as u16 + 4, 0);
        }
        self.fladj = Fladj::new(unsafe { pci_bus.pci_base(pci_device).unwrap().add(0x61) } as u64);
        self.devices = PageAllocator::new(
            &mut self.memory_space,
            self.usbbase.hcsparams().n_ports() as u32,
        );
        self.initialize();
        sleep(100);
        self.information.potential_device_count = self.usbbase.hcsparams().n_ports() as u16;
        return true;
    }

    fn gather_device_information(&mut self) -> bool {
        for i in 0..self.devices.size() {
            let device = self.devices.as_mut(i).unwrap();
            let raw_descriptor = device.get_descriptor(DEVICE_DESCRIPTOR_TYPE, 0, Option::None, 18);
            let device_descriptor = raw_descriptor.as_device_descriptor();

            device
                .default_control_endpoint
                .get_designated_queue_head()
                .set_maximum_packet_length(device_descriptor.b_max_packet_size0 as u16);
            device
                .default_control_endpoint
                .update_max_packet_size(device_descriptor.b_max_packet_size0 as u16);
            device.device_information.device_class = device_descriptor.b_device_class;
            device.device_information.device_sub_class = device_descriptor.b_device_sub_class;
            device.device_information.device_protocol = device_descriptor.b_device_protocol;
            device.device_information.vendor_id = device_descriptor.id_vendor;
            device.device_information.product_id = device_descriptor.id_product;
            device.device_information.manufacturer = device_descriptor.i_manufacturer;
            device.device_information.i_product = device_descriptor.i_product;
            device.device_information.serial_number = device_descriptor.i_serial_number;
            info!(
                &mut self.module,
                "Device {} : class 0x{:x} subclass 0x{:x}\n",
                device.address,
                device.device_information.device_class,
                device.device_information.device_sub_class
            );
            match self
                .memory_space
                .free(&MemoryBlock::new(0x1000, raw_descriptor.data as u64))
            {
                Ok(_) => {}
                Err(_e) => simple_kernel_panic(
                    "Ehci/gather_device_information",
                    "Could not free device descriptor\n",
                ),
            }
        }
        return true;
    }

    fn configure_devices(&mut self) -> bool {
        let mut interface_array: PageAllocator<EhciInterface> =
            PageAllocator::new(&mut self.memory_space, 32);
        let mut endpoint_array: PageAllocator<EhciEndpoint> =
            PageAllocator::new(&mut self.memory_space, 64);
        let mut configurations: PageAllocator<EhciDeviceConfiguration> =
            PageAllocator::new(&mut self.memory_space, 2);
        for d in 0..self.devices.size() {
            let device = self.devices.as_mut(d).unwrap();
            // fetches 9 bytes for wTotalLength first.
            let raw_descriptor0 =
                device.get_descriptor(CONFIGURATION_DESCRIPTOR_TYPE, 0, Option::None, 9);
            let configuration_descriptor0 = raw_descriptor0.as_configuration_descriptor();
            device.set_configuration(configuration_descriptor0.b_configuration_value);
            device.device_information.max_power_ma = configuration_descriptor0.b_max_power;
            device.device_information.num_interfaces = configuration_descriptor0.b_num_interfaces;
            let raw_descriptor1 = device.get_descriptor(
                CONFIGURATION_DESCRIPTOR_TYPE,
                0,
                Option::None,
                configuration_descriptor0.w_total_length,
            );
            device.configurations = configurations.as_mut_ptr(configurations.size()).unwrap();
            device.num_configurations = 1; // this Implementation only supports 1 configuration
            configurations.push_back(EhciDeviceConfiguration::new(
                &mut self.memory_space,
                &mut interface_array,
                &mut endpoint_array,
                unsafe { raw_descriptor1.data.add(9) },
                configuration_descriptor0.b_num_interfaces as u16,
            ));

            // TODO: Implement parser for this! + add EhciEndpoint trait
            match self
                .memory_space
                .free(&MemoryBlock::new(0x1000, raw_descriptor0.data as u64))
            {
                Ok(_) => {}
                Err(_e) => simple_kernel_panic(
                    "Ehci/configure_devices",
                    "Could not free configuration descriptor 0\n",
                ),
            }

            match self
                .memory_space
                .free(&MemoryBlock::new(0x1000, raw_descriptor1.data as u64))
            {
                Ok(_) => {}
                Err(_e) => simple_kernel_panic(
                    "Ehci/configure_devices",
                    "Could not free configuration descriptor 1\n",
                ),
            };
        }
        return true;
    }

    // former: reset_and_address_devices
    fn untraited_work0(&mut self) -> Option<bool> {
        let data_packet_base = match self.memory_space.alloc_zero(1) {
            Ok(mb) => mb.get_base(),
            Err(_e) => simple_kernel_panic(
                "Ehci/initialize_ports",
                "Could not allocate memory for Default Control Endpoint Data Packets\n",
            ),
        };

        if data_packet_base > 0xFFFFFFFF {
            simple_kernel_panic(
                "Ehci/initialize_ports",
                "Allocated memory for Default Control Endpoint Data Packets is above 0xFFFFFFFF\n",
            );
        }
        self.data_packet_base = data_packet_base as u32;
        let mut allready_enabled = false;
        for i in 0..self.usbbase.hcsparams().n_ports() {
            let mut port = self.usbbase.portsc(i);

            if !port.current_connect_status() || !port.connect_status_change() {
                continue;
            }
            // If the Port is not a high speed device -> hand of to Companion controller
            if let LineStatus::K = port.line_status() {
                port.set_port_owner(true);
                continue;
            }
            self.reset_port(i, self.data_packet_base);
            self.data_packet_base += 64;
            *self
                .devices
                .as_mut(self.devices.size() - 1)
                .unwrap()
                .default_control_endpoint
                .get_designated_queue_head() =
                QueueHead::new(self.asynchronous_list.address_of_index(i as u16) as u64);

            let mut current_qh = QueueHead::new(
                self.devices
                    .as_ref(self.devices.size() - 1)
                    .unwrap()
                    .default_control_endpoint
                    .get_designated_queue_head_address() as u64,
            );
            {
                if i == 0 {
                    current_qh.horizontal_link_pointer().set_terminate(true);
                    current_qh.set_head_of_reclaimation_list_flag(true);
                }
                current_qh
                    .horizontal_link_pointer()
                    .set_type(EhciCommonType0::QH);
                current_qh.next_qtd_pointer().set_terminate(true);
                current_qh.set_device_address(0);
                current_qh.set_inactive_on_next_transaction(false);
                current_qh.set_endpoint_number(0);
                current_qh.set_endpoint_speed(EndpointSpeed::HighSpeed);
                current_qh.set_data_toggle_control(false);
                current_qh.set_maximum_packet_length(8);
                current_qh.set_endpoint_control_flag(false);
                current_qh.set_nak_count_reload(0);
                current_qh.set_mult(Mult::OneTransactionPerMicroframe);
                current_qh.set_frame_s_mask(0);
                current_qh.set_frame_s_mask(0);

                if i != 0 {
                    current_qh
                        .horizontal_link_pointer()
                        .set_link_pointer(self.asynchronous_list.address_of_index(0));
                    current_qh.horizontal_link_pointer().set_terminate(false);

                    self.devices
                        .as_mut(self.devices.size() - 2)
                        .unwrap()
                        .default_control_endpoint
                        .get_designated_queue_head()
                        .horizontal_link_pointer()
                        .set_link_pointer(current_qh.get_address());
                    self.devices
                        .as_mut(self.devices.size() - 2)
                        .unwrap()
                        .default_control_endpoint
                        .get_designated_queue_head()
                        .horizontal_link_pointer()
                        .set_terminate(false);
                }
            }

            if i != 0 {
                let current_endpoint_queue_head_address = self
                    .devices
                    .as_ref(self.devices.size() - 1)
                    .unwrap()
                    .default_control_endpoint
                    .get_designated_queue_head_address();

                let current_endpoint_queue_head =
                    QueueHead::new(current_endpoint_queue_head_address as u64);

                let first_endpoint_queue_head_address = self
                    .devices
                    .as_ref(0)
                    .unwrap()
                    .default_control_endpoint
                    .get_designated_queue_head_address();

                let last_endpoint_queue_head = QueueHead::new(
                    self.devices
                        .as_ref(i as u32 - 1)
                        .unwrap()
                        .default_control_endpoint
                        .get_designated_queue_head_address() as u64,
                );

                current_endpoint_queue_head
                    .horizontal_link_pointer()
                    .set_link_pointer(first_endpoint_queue_head_address);
                current_endpoint_queue_head
                    .horizontal_link_pointer()
                    .set_type(EhciCommonType0::QH);
                current_endpoint_queue_head
                    .horizontal_link_pointer()
                    .set_terminate(false);
                last_endpoint_queue_head
                    .horizontal_link_pointer()
                    .set_link_pointer(current_endpoint_queue_head_address);
                last_endpoint_queue_head
                    .horizontal_link_pointer()
                    .set_type(EhciCommonType0::QH);
                last_endpoint_queue_head
                    .horizontal_link_pointer()
                    .set_terminate(false);
            }
            if !allready_enabled {
                self.usbbase.usbcmd().set_asynchronous_schedule_enable(true);
                allready_enabled = true;
            }
            let device = self.devices.as_mut(self.devices.size() - 1).unwrap();
            device.set_address(device.port_num as u16 + 1);
            device.address = device.port_num + 1;
            current_qh.set_device_address(device.address);
        }
        self.usbbase
            .usbintr()
            .set_port_change_interrupt_enable(true);
        self.information.active_device_count = self.devices.size() as u16;
        Option::Some(true)
    }
    fn untraited_work1(&mut self) -> Option<bool> {
        Option::None
    }

    //former: update_async ; Modified
    fn untraited_work2(&mut self) -> Option<bool> {
        self.stop_async();
        let new_async_list = AsynchronousList::new(&mut self.memory_space);

        for d in 0..self.devices.size() {
            let device = self.devices.as_mut(d).unwrap();
            // this is after reset.
            let mut qh = QueueHead::new(new_async_list.address_of_index(d as u16) as u64);
            qh.horizontal_link_pointer().set_type(EhciCommonType0::QH);
            qh.horizontal_link_pointer()
                .set_link_pointer(new_async_list.address_of_index(d as u16 + 1));
            qh.horizontal_link_pointer().set_terminate(false);
            qh.set_device_address(device.address);
            qh.set_endpoint_speed(EndpointSpeed::HighSpeed);
            if d == 0 {
                qh.set_head_of_reclaimation_list_flag(true);
            }
            qh.set_maximum_packet_length(device.default_control_endpoint.get_maximum_packet_size());
            qh.set_mult(Mult::OneTransactionPerMicroframe);
            *device.default_control_endpoint.get_designated_queue_head() = qh;
        }
        let mut index = self.devices.size() as u16;
        let mut last_address_written = 0;
        for d in 0..self.devices.size() {
            let device = self.devices.as_mut(d).unwrap();
            for config in 0..device.num_configurations {
                let configuration =
                    unsafe { device.configurations.add(config as usize).as_ref().unwrap() };
                for i in 0..device.device_information.num_interfaces {
                    let interface = unsafe {
                        configuration
                            .get_interfaces()
                            .add(i as usize)
                            .as_ref()
                            .unwrap()
                    };
                    for ep in 0..interface.endpoint_count() {
                        let endpoint =
                            unsafe { interface.get_endpoints().add(ep as usize).as_mut().unwrap() };
                        let mut qh = QueueHead::new(new_async_list.address_of_index(index) as u64);
                        qh.set_endpoint_speed(EndpointSpeed::HighSpeed);
                        qh.set_device_address(device.address);
                        qh.set_maximum_packet_length(endpoint.get_maximum_packet_size());
                        qh.horizontal_link_pointer().set_terminate(false);
                        qh.horizontal_link_pointer().set_type(EhciCommonType0::QH);
                        qh.horizontal_link_pointer()
                            .set_link_pointer(new_async_list.address_of_index(index + 1));
                        qh.set_mult(Mult::OneTransactionPerMicroframe);
                        qh.set_endpoint_number(endpoint.endpoint_number());
                        *endpoint.get_designated_queue_head() = qh;
                        last_address_written = new_async_list.address_of_index(index) as u64;
                        index += 1;
                    }
                }
            }
        }
        // Constructs a round robin list
        QueueHead::new(last_address_written)
            .horizontal_link_pointer()
            .set_link_pointer(new_async_list.address_of_index(0));
        match self.memory_space.free(&MemoryBlock::new(
            0x1000,
            self.asynchronous_list.address_of_index(0) as u64,
        )) {
            Ok(_) => {}
            Err(_e) => simple_kernel_panic("Ehci/update_async", "Could not free old async list\n"),
        };
        self.asynchronous_list = new_async_list;
        self.asynchronous_list.set(self);
        self.start_async();
        self.start_periodic();
        Option::Some(true)
    }

    fn get_device(&self, index: u16) -> Option<&dyn crate::drivers::usb::traits::UsbDevice> {
        if index > self.devices.size() as u16 {
            None
        } else {
            unsafe {
                let ptr = self.devices.as_ref_ptr(index as u32).unwrap();
                let ret = &*ptr;
                Some(ret)
            }
        }
    }

    fn get_mut_device(&mut self, index: u16) -> Option<&mut dyn UsbDevice> {
        if index > self.devices.size() as u16 {
            None
        } else {
            unsafe {
                let ptr = self.devices.as_mut_ptr(index as u32).unwrap();
                let ret = &mut *ptr;
                Some(ret)
            }
        }
    }

    fn install_interrupt_poller(
        &mut self,
        device: &dyn UsbDevice,
        endpoint: &dyn UsbEndpoint,
        frame: u8,
        report_address: u32,
        bytes_to_transfer: u16,
        callback: Option<UsbInterruptPollerCallbackFn>,
    ) {
        if !self.periodic_list.get_element(frame as u16).get_terminate() {
            simple_kernel_panic(
                "Ehci/install_interrupt_poller",
                "Unimplemented appending of QH\n",
            );
            //TODO: append QH
        } else {
            let qh_address = self.interrupt_pollers_qhs;
            self.interrupt_pollers_qhs = unsafe {
                self.interrupt_pollers_qhs
                    .add(QueueHead::SIZE as usize + 16) // Rounds up to 64 which is aligned
            };
            let mut qtds = [
                QueueElementTransferDescriptor::new(self.interrupt_pollers_qtds as u64),
                QueueElementTransferDescriptor::new(
                    unsafe { self.interrupt_pollers_qtds.add(32) } as u64
                ),
            ];
            self.interrupt_pollers_qtds = unsafe { self.interrupt_pollers_qtds.add(64) };

            for i in 0..2 {
                qtds[i].initialize(Option::None, PidCode::InToken, bytes_to_transfer, true);
                qtds[i].set_status_bit(7); // Activate
                qtds[i].set_current_offset((report_address & 0xFFF) as u16);
                qtds[i].set_buffer_pointer0((report_address >> 12) << 12);
            }

            let mut qh = QueueHead::new(qh_address as u64);
            qh.high_speed_initialize(
                endpoint.endpoint_number(),
                device.device_address(),
                endpoint.get_maximum_packet_size(),
                true,
                Option::None,
                &qtds[0],
                Mult::OneTransactionPerMicroframe,
            );
            qh.set_frame_s_mask(1); // Tells the controller to interrupt at the 0th micro-Frame of the Frame.
            qh.set_status_bit(7); // Activate

            let mut val = 0u16;
            while 1024 >= val + frame as u16 {
                self.periodic_list
                    .set_element(val, qh_address as u32, EhciCommonType0::QH);
                val += frame as u16;
            }

            let designated_qhs = self
                .interrupt_pollers_designated_qhs
                .as_mut_ptr(self.interrupt_pollers_designated_qhs.size())
                .unwrap();
            self.interrupt_pollers_designated_qhs.push_back(qh.clone());
            self.interrupt_pollers.push_back(EhciInterruptPoller {
                callback,
                designated_qhs,
                queue_head_count: 1,
                transfer_size: bytes_to_transfer,
                designated_qtds: qtds,
                current_active: 0,
            });
        }
    }

    /**
     *  Software can only set Run/Stop to 1, if HCHalted is 1
     */
    fn start(&mut self) {
        if self.usbbase.usbsts().hchalted() {
            self.usbbase.usbcmd().set_rs(true);
        }
        //Waits, until the Controller is running
        while self.usbbase.usbsts().hchalted() {}
    }

    /**
     * Software can only set Run/Stop to 0, if HCHalted is 0
     */
    fn stop(&mut self) {
        if !self.usbbase.usbsts().hchalted() {
            self.usbbase.usbcmd().set_rs(false);
        }
        //Waits until the Controller is stopped
        while !self.usbbase.usbsts().hchalted() {}
    }

    fn error_present(&self) -> bool {
        return (self.usbbase.usbsts().as_u32() & (1 << 1 | 1 << 4)) != 0;
    }
    fn number_of_active_devices(&self) -> u16 {
        return self.information.active_device_count;
    }
    fn number_of_potential_devices(&self) -> u16 {
        return self.information.potential_device_count;
    }
}
