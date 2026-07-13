use core::{ffi::c_void, slice};

use crate::{
    drivers::usb::{
        ehci::{
            Ehci, EhciInterruptPoller,
            configuration_parser::EhciDeviceConfiguration,
            data_structures::{
                AsynchronousList, EhciLinkType, EndpointSpeed, Mult,
                QueueElementTransferDescriptor, QueueHead, QueueHeadBitPart,
                QueueHeadPart::{self, Eps, MaximumPacketLength, Type},
            },
            registers::{
                Fladj,
                HccParamsPart::Eecp,
                HcsParamsPart::{self, NPorts},
                LineStatus,
                PortScBitPart::{self, ConnectStatusChange, CurrentConnectStatus},
                PortScPart, UsbBase,
                UsbCmdBitPart::{AsynchronousScheduleEnable, Rs},
                UsbIntrBitPart::PortChangeInterruptEnable,
                UsbStsBitPart::{self, HcHalted},
            },
            structures::{device::EhciDevice, endpoint::EhciEndpoint, interface::EhciInterface},
        },
        independent::{
            PidCode, UsbControllerType, UsbDescriptorType, UsbDeviceState, UsbRequestCode,
        },
        standard_requests::{UsbConfigurationDescriptor, UsbDeviceStandardRequest},
        traits::{
            UsbConfiguration, UsbController, UsbDevice, UsbEndpoint, UsbInterface,
            UsbInterruptPollerCallbackFn,
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

        if self.usbbase.hccparams().get(Eecp) >= 0x40 {
            let eecp = self.usbbase.hccparams().get(Eecp);
            let val = pci_bus.read_configuration_space_u32(pci_device, eecp as u16);
            pci_bus.write_configuration_space_u32(pci_device, eecp as u16, val | 1 << 24);
            sleep(40);
            pci_bus.write_configuration_space_u32(pci_device, eecp as u16 + 4, 0);
        }
        self.fladj = Fladj::new(unsafe { pci_bus.pci_base(pci_device).unwrap().add(0x61) } as u64);
        let ports = self.usbbase.hcsparams().get(HcsParamsPart::NPorts);
        let device_memory = match self.memory_space.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(
                self.module.name(),
                "Could not allocate Memory for Device List\n",
            ),
        };
        self.devices =
            unsafe { slice::from_raw_parts_mut(device_memory.as_mut_ptr(), ports as usize) };
        for (port_index, device) in self.devices.iter_mut().enumerate() {
            *device = EhciDevice::new_detached(port_index as u8);
        }
        self.information.potential_device_count = ports as u16;
        self.initialize();
        sleep(40);
        return true;
    }

    fn gather_device_information(&mut self) -> bool {
        for device in &mut *self.devices {
            if let UsbDeviceState::Detached = device.state {
                continue;
            }

            let raw_descriptor =
                device.get_descriptor(UsbDescriptorType::Device, 0, Option::None, 18);
            let device_descriptor = raw_descriptor.as_device_descriptor();

            device
                .default_control_endpoint
                .get_designated_queue_head()
                .set_part(
                    MaximumPacketLength,
                    device_descriptor.b_max_packet_size0 as u32,
                );
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
            if let Result::Err(_) = self.memory_space.free(&raw_descriptor.data) {
                simple_kernel_panic(self.module.name(), "Could not free device descriptor\n")
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
            PageAllocator::new(&mut self.memory_space, 16);
        for device in &mut *self.devices {
            if let UsbDeviceState::Detached = device.state {
                continue;
            }
            // fetches 9 bytes for wTotalLength first.
            let raw_descriptor0 =
                device.get_descriptor(UsbDescriptorType::Configuration, 0, Option::None, 9);
            let configuration_descriptor0 = raw_descriptor0.as_configuration_descriptor();
            device.set_configuration(configuration_descriptor0.b_configuration_value);
            device.device_information.max_power_ma = configuration_descriptor0.b_max_power as u16;
            device.device_information.num_interfaces = configuration_descriptor0.b_num_interfaces;
            let raw_descriptor1 = device.get_descriptor(
                UsbDescriptorType::Configuration,
                0,
                Option::None,
                configuration_descriptor0.w_total_length,
            );
            device.configurations = unsafe {
                slice::from_raw_parts_mut(
                    configurations.as_mut_ptr(configurations.size()).unwrap(),
                    1,
                )
            };
            device.num_configurations = 1; // this Implementation only supports 1 configuration
            configurations.push_back(EhciDeviceConfiguration::new(
                &mut self.memory_space,
                &mut interface_array,
                &mut endpoint_array,
                unsafe { raw_descriptor1.data.as_ptr::<c_void>().add(9) },
                configuration_descriptor0.b_num_interfaces as u16,
            ));

            if let Result::Err(_) = self.memory_space.free(&raw_descriptor0.data) {
                simple_kernel_panic(
                    self.module.name(),
                    "Could not free configuration descriptor\n",
                )
            }
            if let Result::Err(_) = self.memory_space.free(&raw_descriptor1.data) {
                simple_kernel_panic(
                    self.module.name(),
                    "Could not free configuration descriptor\n",
                )
            }
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
        /* Notice: on 'continue' device is initialized as Detached by initialize_controller*/
        for i in 0..self.usbbase.hcsparams().get(NPorts) as u8 {
            let mut port = self.usbbase.portsc(i);

            if !port.is_set(CurrentConnectStatus) || !port.is_set(ConnectStatusChange) {
                continue;
            }
            // If the Port is not a high speed device -> hand of to Companion controller
            if let LineStatus::K = LineStatus::new(port.get(PortScPart::LineStatus)) {
                port.set(PortScBitPart::PortOwner, true);
                continue;
            }
            if !self.reset_port(i, self.data_packet_base) {
                continue;
            }
            self.data_packet_base += 64;

            *self.devices[i as usize]
                .default_control_endpoint
                .get_designated_queue_head() = self
                .asynchronous_list
                .index_to_qh(self.information.active_device_count as u16);

            let mut current_qh = QueueHead::new(
                self.devices[i as usize]
                    .default_control_endpoint
                    .get_designated_queue_head_address() as u64,
            );
            {
                /*
                 * This will set
                 *  device address = 0
                 *  endpoint number = 0
                 *  inactive on next transaction = false
                 *  data toggle control = false
                 *  endpoint control flag = false
                 *  Nak count reload = 0
                 *  Mikro Frame S Mask = 0
                 *  Mikro Frame C Mask = 0
                 */
                current_qh.reset();
                if self.information.active_device_count == 0 {
                    current_qh.set(QueueHeadBitPart::T, true);
                    current_qh.set(QueueHeadBitPart::H, true);
                }
                current_qh.set_part(Type, EhciLinkType::Qh as u32);
                current_qh.set_part(Eps, EndpointSpeed::HighSpeed as u32);
                current_qh.set_part(MaximumPacketLength, 8);
                current_qh.set_part(
                    QueueHeadPart::Mult,
                    Mult::OneTransactionPerMicroframe as u32,
                );
                current_qh.next_qtd_pointer().set_terminate(true);

                if self.information.active_device_count != 0 {
                    current_qh
                        .set_horizontal_link_pointer(self.asynchronous_list.address_of_index(0));
                    current_qh.set(QueueHeadBitPart::T, false);

                    for j in i - 1..=0 {
                        if let UsbDeviceState::Address = self.devices[j as usize].state {
                            self.devices[j as usize]
                                .default_control_endpoint
                                .get_designated_queue_head()
                                .chain_next_qh(current_qh.get_address());
                            break;
                        }
                    }
                }
            }

            if self.information.active_device_count != 0 {
                let current_endpoint_queue_head_address = self.devices[i as usize]
                    .default_control_endpoint
                    .get_designated_queue_head_address();

                let mut current_endpoint_queue_head =
                    QueueHead::new(current_endpoint_queue_head_address as u64);

                let mut first_endpoint_queue_head_address = 0;

                for j in 0..i - 1 {
                    if let UsbDeviceState::Address = self.devices[j as usize].state {
                        first_endpoint_queue_head_address = self.devices[j as usize]
                            .default_control_endpoint
                            .get_designated_queue_head_address();
                        break;
                    }
                }
                assert_ne!(first_endpoint_queue_head_address, 0);

                let mut last_endpoint_queue_head: QueueHead = QueueHead::new(0);

                for j in i - 1..=0 {
                    if let UsbDeviceState::Address = self.devices[j as usize].state {
                        last_endpoint_queue_head = QueueHead::new(
                            self.devices[j as usize]
                                .default_control_endpoint
                                .get_designated_queue_head_address()
                                as u64,
                        );
                        break;
                    }
                }
                assert_ne!(last_endpoint_queue_head.get_address(), 0);
                current_endpoint_queue_head.chain_next_qh(first_endpoint_queue_head_address);
                last_endpoint_queue_head.chain_next_qh(current_endpoint_queue_head_address);
            }
            let mut current_device = &mut self.devices[i as usize];
            if !allready_enabled {
                self.usbbase.usbcmd().set(AsynchronousScheduleEnable, true);
                allready_enabled = true;
            }
            current_device.set_address(current_device.port_num as u16 + 1);
            current_device.address = current_device.port_num + 1;
            current_device.state = UsbDeviceState::Address;
            current_qh.set_part(QueueHeadPart::DeviceAddress, current_device.address as u32);
            self.information.active_device_count += 1;
        }
        self.usbbase.usbintr().set(PortChangeInterruptEnable);
        Option::Some(true)
    }
    fn untraited_work1(&mut self) -> Option<bool> {
        Option::None
    }

    //former: update_async ; Modified
    fn untraited_work2(&mut self) -> Option<bool> {
        self.stop_async();
        let new_async_list = AsynchronousList::new(&mut self.memory_space);

        let mut device_inserted = 0u16;

        for (device_index, device) in self.devices.iter_mut().enumerate() {
            if let UsbDeviceState::Detached = device.state {
                continue;
            }
            // this is after reset.
            let mut qh = new_async_list.index_to_qh(device_inserted);
            if device_inserted == 0 {
                qh.set(super::data_structures::QueueHeadBitPart::H, true);
            }
            qh.set_common_info(
                EndpointSpeed::HighSpeed,
                device.address,
                device.default_control_endpoint.get_maximum_packet_size(),
                0, // Since it´s the default control endpoint
                Mult::OneTransactionPerMicroframe,
            );

            qh.chain_next_qh(new_async_list.address_of_index(device_inserted + 1));
            *device.default_control_endpoint.get_designated_queue_head() = qh;
            device_inserted += 1;
        }

        let mut index = device_inserted;
        let mut last_address_written = 0;
        for device in &mut *self.devices {
            if let UsbDeviceState::Detached = device.state {
                continue;
            }
            for configuration in &mut *device.configurations {
                for i in 0..configuration.get_interface_count() {
                    let interface = configuration.get_mut_interface(i).unwrap();
                    for ep in 0..interface.endpoint_count() {
                        let endpoint = unsafe {
                            &mut *(interface.get_mut_endpoint(ep).unwrap() as *mut dyn UsbEndpoint
                                as *mut EhciEndpoint)
                        };
                        let mut qh = QueueHead::new(new_async_list.address_of_index(index) as u64);
                        qh.set_common_info(
                            EndpointSpeed::HighSpeed,
                            device.address,
                            endpoint.get_maximum_packet_size(),
                            endpoint.endpoint_number(),
                            Mult::OneTransactionPerMicroframe,
                        );
                        qh.chain_next_qh(new_async_list.address_of_index(index + 1));
                        *endpoint.get_designated_queue_head() = qh;
                        last_address_written = new_async_list.address_of_index(index) as u64;
                        index += 1;
                    }
                }
            }
        }

        // Constructs a round robin list

        let mut first_addr = 0;

        for device in &mut *self.devices {
            if let UsbDeviceState::Address = device.state {
                first_addr = device
                    .default_control_endpoint
                    .get_designated_queue_head_address();
                break;
            }
        }

        if first_addr == 0 {
            if let Result::Err(_) = self.memory_space.free(&MemoryBlock::new(
                0x1000,
                new_async_list.address_of_index(0) as u64,
            )) {
                simple_kernel_panic(self.module.name(), "Could not free new async list\n")
            }
            self.dummy = true;
            return Option::Some(true);
        }
        QueueHead::new(last_address_written).set_horizontal_link_pointer(first_addr);
        match self.memory_space.free(&MemoryBlock::new(
            0x1000,
            self.asynchronous_list.address_of_index(0) as u64,
        )) {
            Ok(_) => {}
            Err(_e) => simple_kernel_panic("Ehci/update_async", "Could not free old async list\n"),
        };
        self.asynchronous_list = new_async_list;
        self.asynchronous_list
            .set(&mut self.usbbase.asynclistaddr());
        self.start_async();
        self.start_periodic();
        Option::Some(true)
    }

    fn get_device(&self, index: u16) -> Option<&dyn crate::drivers::usb::traits::UsbDevice> {
        if index > self.devices.len() as u16 {
            None
        } else {
            return Some(&self.devices[index as usize]);
        }
    }

    fn get_mut_device(&mut self, index: u16) -> Option<&mut dyn UsbDevice> {
        if index > self.devices.len() as u16 {
            None
        } else {
            return Some(&mut self.devices[index as usize]);
        }
    }

    fn install_interrupt_poller(
        &mut self,
        device: &mut dyn UsbDevice,
        interface_index: u8,
        endpoint_index: u8,
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

            let endpoint = device
                .get_configuration(0)
                .unwrap()
                .get_interface(interface_index)
                .unwrap()
                .get_endpoint(endpoint_index as u16)
                .unwrap();

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
            qh.set_part(QueueHeadPart::MikroFrameSMask, 1); // Tells the controller to interrupt at the 0th micro-Frame of the Frame.
            qh.set_status_bit(7); // Activate

            let mut val = 0u16;
            while 1024 >= val + frame as u16 {
                self.periodic_list
                    .set_element(val, qh_address as u32, EhciLinkType::Qh);
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
        if self.usbbase.usbsts().is_set(HcHalted) {
            self.usbbase.usbcmd().set(Rs, true);
        }
        //Waits, until the Controller is running
        while self.usbbase.usbsts().is_set(HcHalted) {}
    }

    /**
     * Software can only set Run/Stop to 0, if HCHalted is 0
     */
    fn stop(&mut self) {
        if !self.usbbase.usbsts().is_set(HcHalted) {
            self.usbbase.usbcmd().set(Rs, false);
        }
        //Waits until the Controller is stopped
        while !self.usbbase.usbsts().is_set(HcHalted) {}
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
