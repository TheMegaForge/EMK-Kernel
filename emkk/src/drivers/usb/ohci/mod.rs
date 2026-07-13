use core::{
    arch::asm, ffi::c_void, mem, ops::Range, ptr::{self, null, null_mut}, slice,
};

use crate::{
    arch::{isr::ISRRegisters, lapic::LocalApic}, drivers::usb::{
        independent::{Direction, UsbControllerType, UsbDescriptorType, UsbDeviceInformation, UsbDeviceState, UsbTransferType}, ohci::{configuration_parser::OhciDeviceConfiguration, data_structures::{HostControllerFunctionalState, OhciBar, OhciCommandStatusBitPart, OhciHcca, OhciInterrupt, OhciRhDescriptorABitPart::Nps, OhciRhDescriptorBPart::Ppcm, OhciRhPortStatusBitPart::{self, Pes, Prs}, OhciRhStatusBitPart::Lpsc}, structures::{OHCI_TRANSFER_DESCRIPTOR_PROCESSED, device::OhciDevice, endpoint::{EndpointDescriptorBitPart::{self, F, K, S}, EndpointDescriptorPart::{self, Mps}, OhciEndpointDescriptor, OhciGeneralEndpoint, OhciGeneralEndpointRealEndpoint::{self, Unassigned}, OhciNonPeriodicEndpoint, OhciPeriodicEndpoint}, interface::OhciInterface, interrupt_list::OhciHccaInterruptList, non_periodic_list::OhciNonPeriodicList, transfer_descriptors::{GeneralTD, GeneralTDBitPart, GeneralTDPart, IsochTD, RawGeneralTD, RawIsochTD}}}, standard_requests::{UsbDeviceStandardRequest, UsbHID}, traits::{UsbConfiguration, UsbController, UsbDevice, UsbEndpoint, UsbInterruptPollerCallbackFn}
    }, fixed_vaddrs::{OHCI_BAR_FIXED_VADDR, ref_processor_mut}, hal::{
        memory::{
            allocator::{Allocator, MemoryBlock},
            pager::{PAGER_PCD, PAGER_PRESENT, PAGER_RW, Page},
        },
        pci_bus::{PciBarIndex, PciBus},
        print::{Module, simple_kernel_panic},
    }, info, success, time::sleep, utils::{allocators::PageAllocator, memory::alloc_zero_or_crash, slices::{invalid_mut_slice, resize_slice, slice_end_address}, traits::Region}, warn
};

pub mod data_structures;
pub mod structures;
pub mod request_impl;
pub mod configuration_parser;

pub struct OhciControllerTds<'a> {
    memory: [MemoryBlock; 4],
    control_tds: &'a mut [RawGeneralTD],
    bulk_tds: &'a mut [RawGeneralTD],
    interrupt_tds: &'a mut [RawGeneralTD],
    isochronous_tds: &'a mut [RawIsochTD],

    allocated_control_tds: u16,
    allocated_bulk_tds: u16,
    allocated_interrupt_tds: u8,
    allocated_isochronous_tds: u16
}

impl<'a> OhciControllerTds<'a> {
    pub const CONTROL_TD_MEMORY_INDEX: usize = 0;
    pub const BULK_TD_MEMORY_INDEX: usize = 1;
    pub const INTERRUPT_TD_MEMORY_INDEX: usize = 2;
    pub const ISOCHRONOUS_TD_MEMORY_INDEX: usize = 3;

    pub const fn empty() -> Self {
        return Self {
            memory: [MemoryBlock::empty(); 4],
            control_tds: invalid_mut_slice(),
            bulk_tds: invalid_mut_slice(),
            interrupt_tds: invalid_mut_slice(),
            isochronous_tds: invalid_mut_slice(),
            allocated_control_tds: 0,
            allocated_bulk_tds: 0,
            allocated_interrupt_tds: 0,
            allocated_isochronous_tds: 0
        };
    }

    pub fn new(module: &mut Module<'static>, physical_allocator: &mut Allocator) -> Self {

        let control_td_memory = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(module.name(), "Could not allocate memory for control transfer descriptor Array\n"),
        };
        let bulk_td_memory = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(module.name(), "Could not allocate memory for bulk transfer descriptor Array\n"),
        };
        let interrupt_td_memory = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(module.name(), "Could not allocate memory for interrupt transfer descriptor Array\n"),
        };
        let isochronous_td_memory = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(module.name(), "Could not alllocate memory for isochronous transfer descriptor Array\n"),
        };

        unsafe {
            return Self {
                control_tds: slice::from_raw_parts_mut(control_td_memory.as_mut_ptr(), 0x1000/size_of::<RawGeneralTD>()),
                bulk_tds: slice::from_raw_parts_mut(bulk_td_memory.as_mut_ptr(), 0x1000/size_of::<RawGeneralTD>()),
                interrupt_tds: slice::from_raw_parts_mut(interrupt_td_memory.as_mut_ptr(), 0x1000/size_of::<RawGeneralTD>()),
                isochronous_tds: slice::from_raw_parts_mut(isochronous_td_memory.as_mut_ptr(), 0x1000/size_of::<RawIsochTD>()),
                allocated_control_tds: 0,
                allocated_bulk_tds: 0,
                allocated_interrupt_tds: 0,
                allocated_isochronous_tds: 0,
                memory: [
                    control_td_memory,
                    bulk_td_memory,
                    interrupt_td_memory,
                    isochronous_td_memory
                ],
            };
        }
    }

}

pub struct OhciControllerEndpoints<'a> {
    memory: [MemoryBlock; 2],
    non_periodic_endpoints: &'a mut [OhciNonPeriodicEndpoint],
    periodic_endpoints: &'a mut [OhciPeriodicEndpoint],
    general_endpoints: PageAllocator<OhciGeneralEndpoint>,

    allocated_non_periodic_endpoints: u16,
    allocated_periodic_endpoints: u16
}

impl<'a> OhciControllerEndpoints<'a> {
    pub const NON_PERIODIC_ENDPOINT_MEMORY_INDEX: usize = 0;
    pub const PERIODIC_ENDPOINT_MEMORY_INDEX: usize = 1;

    pub const fn empty() -> Self {
        return Self {
            memory: [MemoryBlock::empty();2],
            non_periodic_endpoints: invalid_mut_slice(),
            periodic_endpoints: invalid_mut_slice(),
            general_endpoints: PageAllocator::empty(),
            allocated_non_periodic_endpoints: 0,
            allocated_periodic_endpoints: 0
        };
    }

    pub fn new(module: &mut Module<'static>, physical_allocator: &mut Allocator) -> Self {

        let non_periodic_endpoint_memory = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(module.name(), "Could not allocate memory for non periodic endpoint Array\n"),
        };
        let periodic_endpoint_memory = match physical_allocator.alloc_zero(1) {
            Ok(mb) => mb,
            Err(_e) => simple_kernel_panic(module.name(), "Could not allocate memory for periodic endpoint Array\n"),
        };

        return Self {
            non_periodic_endpoints: unsafe { slice::from_raw_parts_mut(non_periodic_endpoint_memory.as_mut_ptr(), 0x1000/size_of::<OhciNonPeriodicEndpoint>()) },
            periodic_endpoints: unsafe { slice::from_raw_parts_mut(periodic_endpoint_memory.as_mut_ptr(), 0x1000/size_of::<OhciPeriodicEndpoint>()) },
            general_endpoints: PageAllocator::new(physical_allocator, 256),
            allocated_non_periodic_endpoints: 0,
            allocated_periodic_endpoints: 0,
            memory: [
                non_periodic_endpoint_memory,
                periodic_endpoint_memory
            ],
        };
    }

}

pub struct OhciInterruptPoller {
    callback: UsbInterruptPollerCallbackFn,
    td_offset: u8
}

pub struct OhciController {
    bar: OhciBar,
    hcca: OhciHcca,
    interrupt_list: OhciHccaInterruptList,
    bulk_list: OhciNonPeriodicList,
    control_list: OhciNonPeriodicList,
    present: bool,
    private_physical_allocator: Allocator,
    num_potential_devices: u8,
    num_active_devices: u8,
    error_present: bool,
    /**
     * Size = num_potential_devices
     * But only num_active_devices are valid
     */
    device_memory: MemoryBlock,
    hid_memory: MemoryBlock,
    interrupt_pollers_memory: MemoryBlock,

    devices: &'static mut [OhciDevice],
    module: Module<'static>,
    interfaces: PageAllocator<OhciInterface>,
    hid_descriptors: &'static mut [UsbHID],

    transfer_descriptors: OhciControllerTds<'static>,
    endpoints: OhciControllerEndpoints<'static>,

    interrupt_pollers: &'static mut [OhciInterruptPoller]

}

impl OhciController {
    pub const fn empty() -> Self {
        return Self {
            bar: OhciBar::empty(),
            present: false,
            private_physical_allocator: Allocator::empty(),
            hcca: OhciHcca::new(null_mut()),
            interrupt_list: OhciHccaInterruptList::new(null_mut()),
            num_potential_devices: 0,
            num_active_devices: 0,
            error_present: false,
            bulk_list:  OhciNonPeriodicList::new(null_mut(), null_mut(), null_mut(), 0),
            control_list: OhciNonPeriodicList::new(null_mut(), null_mut(), null_mut(), 0),
            devices: invalid_mut_slice(),
            device_memory: MemoryBlock::empty(),
            hid_memory: MemoryBlock::empty(),
            interrupt_pollers_memory: MemoryBlock::empty(),
            module: Module::new("Ohci"),
            interfaces: PageAllocator::empty(),
            hid_descriptors: invalid_mut_slice(),
            endpoints: OhciControllerEndpoints::empty(),
            transfer_descriptors: OhciControllerTds::empty(),
            interrupt_pollers: invalid_mut_slice()
        };
    }
}

pub(in crate::drivers::usb) static mut OHCI_CONTROLLER: OhciController = OhciController::empty();

fn ohci_interrupt(_: &ISRRegisters) {
    #[allow(static_mut_refs)]
    let ohci_controller = unsafe { &mut OHCI_CONTROLLER };

    let mut status = ohci_controller.bar.hc_interrupt_status().as_u32();

    status &= 1 << 30 | 1 << 1 | 1 << 6 | 1 << 4;

    match status {
        2 => {
            let mut curr = (ohci_controller.hcca.done_head() & !1) as *mut u32;

            while !curr.is_null() {
                if curr.addr() >= ohci_controller.transfer_descriptors.control_tds.as_mut_ptr().addr() as usize &&
                    slice_end_address(&ohci_controller.transfer_descriptors.control_tds) as usize >= curr.addr() {
                    /* Is control transfer*/
                    for device in &mut *ohci_controller.devices {
                        let mut desc = device.control_ep.get_endpoint_descriptor();
                        if !desc.is_set(K) {
                            let number_transfer_descriptors = (desc.tail_p() >> 20) & 0xFF;
                            /* Minus 1, since we´re alredy in one of those*/
                            for i in 0..number_transfer_descriptors{
                                curr = unsafe { curr.add(2).read_volatile() } as *mut u32;
                            }
                            desc.set(EndpointDescriptorBitPart::K, true);
                            device.signal();
                            break;
                        }
                    }
                }else if curr.addr() >= ohci_controller.transfer_descriptors.interrupt_tds.as_mut_ptr().addr() as usize &&
                slice_end_address(&ohci_controller.transfer_descriptors.interrupt_tds) as usize >= curr.addr() {
                    /* Is interrupt transfer*/
                    for i in 0..ohci_controller.endpoints.allocated_periodic_endpoints {
                        let mut ed = ohci_controller.endpoints.periodic_endpoints[i as usize].get_endpoint_descriptor();
                        if ed.is_set(F) || ed.head_p() != ed.tail_p() || ed.tail_p() == 0{
                            continue;
                        }
                        let tail_p = ed.tail_p();
                        let td_offset = (tail_p >> 20) & 0xFF;

                        for poller in &*ohci_controller.interrupt_pollers {
                            if poller.td_offset == td_offset as u8{
                                (poller.callback)(ohci_controller);
                                break;
                            }
                        }

                        let size = (tail_p >> 4) & 0xFFFF;
                        curr = unsafe { curr.add(2).read_volatile() } as *mut u32;
                        let mut td = GeneralTD::new((&raw mut ohci_controller.transfer_descriptors.interrupt_tds[td_offset as usize]) as *mut u32);
                        td.write_cbp(td.buffer_end() - (size - 1));
                        td.write_next_td(ed.tail_p());
                        ed.write_head_p(td.address());
                    }
                }
            }
            ohci_controller.bar.hc_interrupt_status().set(OhciInterrupt::Wdh); // clears Interrupt
        }
        8 => {
            todo!("Implement Resume detected interrupt response\n")
        }
        16 => {
            simple_kernel_panic("Ohci/Interrupt", "Unrecoverable Error happend!\n")
        }
        64 => {
            todo!("Implement Root Hub Status Change interrupt response\n")
        }
        0x40000000 => {
            todo!("Implement Ownership change interrupt response\n")
        }
        _ => simple_kernel_panic("Ohci/Interrupt", "Invalid value\n")
    }

    LocalApic::from_local_core().send_eoi();
}

impl UsbController for OhciController {
    fn identity(&self) -> UsbControllerType {
        return UsbControllerType::OHC;
    }
    fn initialize_controller(
        &mut self,
        pci_bus: &PciBus,
        allocator: &mut Allocator,
        isr_vector: u8,
        pci_device: u64,
    ) -> bool {
        self.module = Module::new("Ohci");

        let bar0 = match pci_bus.get_bar(pci_device, PciBarIndex::Index0) {
            Some(bar) => bar,
            None => simple_kernel_panic(self.module.name(), "Could not get bar0\n"),
        };

        let mut pages = bar0.get_length() / 0x1000;
        if bar0.get_length() % 0x1000 != 0 {
            pages += 1;
        }

        match ref_processor_mut().ref_mut_pager().page_4_kb(
            OHCI_BAR_FIXED_VADDR,
            bar0.get_address(),
            PAGER_RW | PAGER_PRESENT | PAGER_PCD,
            allocator,
        ) {
            Ok(_) => {}
            Err(_e) => simple_kernel_panic(self.module.name(), "Could not page bar\n"),
        };

        ref_processor_mut().install_isr(ohci_interrupt, isr_vector);

        let bar = OhciBar::new(OHCI_BAR_FIXED_VADDR as *mut c_void);
        self.bar = bar;
        self.present = true;
        self.private_physical_allocator = allocator.subdivide(32); /* 131.072 bytes for the ohci driver*/

        self.initalize();
        return true;
    }

    fn configure_devices(&mut self) -> bool {
        let mut current_index = 0;
        for device in &mut *self.devices {
            if let UsbDeviceState::Address = device.state {
                let total_length;
                {
                    let raw_descriptor = device.get_descriptor(UsbDescriptorType::Configuration, 0, Option::None, 9);
                    let configuration_descriptor = raw_descriptor.as_configuration_descriptor();
                    device.set_configuration(configuration_descriptor.b_configuration_value);
                    device.device_information.max_power_ma = configuration_descriptor.b_max_power as u16;
                    device.device_information.num_interfaces = configuration_descriptor.b_num_interfaces;
                    total_length = configuration_descriptor.w_total_length;
                    if let Result::Err(_) = self.private_physical_allocator.free(&raw_descriptor.data) {
                        simple_kernel_panic(self.module.name(), "Could not free configuration descriptor\n")
                    }
                }

                {
                    let raw_descriptor = device.get_descriptor(UsbDescriptorType::Configuration, 0, Option::None, total_length);
                    let configuration_descriptor = raw_descriptor.as_configuration_descriptor();

                    let ptr = unsafe { self.hid_descriptors.as_mut_ptr().add(current_index as usize) };
                    device.configuration = OhciDeviceConfiguration::new(
                        unsafe { slice::from_raw_parts_mut(ptr, self.hid_descriptors.len() - current_index) },
                        &mut self.interfaces,
                        &mut self.endpoints.general_endpoints,
                        unsafe { raw_descriptor.data.as_ptr::<c_void>().add(9) },
                        configuration_descriptor.b_num_interfaces as u16
                    );
                    current_index += device.configuration.get_hid_interface_count() as usize;
                    if let Result::Err(_) = self.private_physical_allocator.free(&raw_descriptor.data) {
                        simple_kernel_panic(self.module.name(), "Could not free configuration descriptor\n")
                    }
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
                    let raw_device_descriptor = device.get_descriptor(UsbDescriptorType::Device, 0, Option::None, 8);
                    let device_descriptor = raw_device_descriptor.as_device_descriptor();
                    device.control_ep.get_endpoint_descriptor().set_part(EndpointDescriptorPart::Mps, device_descriptor.b_max_packet_size0 as u32);
                    device.control_ep.update_max_packet_size(device_descriptor.b_max_packet_size0 as u16);
                    if let Result::Err(_) = self.private_physical_allocator.free(&raw_device_descriptor.data) {
                        simple_kernel_panic(self.module.name(), "Could not free device descriptor\n")
                    }
                }
                {
                    let raw_device_descriptor = device.get_descriptor(UsbDescriptorType::Device, 0, Option::None, 18);
                    let device_descriptor = raw_device_descriptor.as_device_descriptor();

                    device.device_information = UsbDeviceInformation {
                        device_class: device_descriptor.b_device_class,
                        device_sub_class: device_descriptor.b_device_sub_class,
                        device_protocol: device_descriptor.b_device_protocol,
                        vendor_id: device_descriptor.id_vendor,
                        product_id: device_descriptor.id_product,
                        manufacturer: device_descriptor.i_manufacturer,
                        i_product: device_descriptor.i_product,
                        serial_number: device_descriptor.i_serial_number,
                        max_power_ma: 0,
                        num_interfaces: 0
                    };
                    if let Result::Err(_) = self.private_physical_allocator.free(&raw_device_descriptor.data) {
                        simple_kernel_panic(self.module.name(), "Could not free device descriptor\n")
                    }
                    info!(
                        &mut self.module,
                        "Device {} is of class {} and subclass {}\n",
                        device.control_ep.get_endpoint_descriptor().get_part(EndpointDescriptorPart::Fa),
                        device.device_information.device_class,
                        device.device_information.device_sub_class
                    );
                }

            }
        }
        return true;
    }
    fn get_device(&self, index: u16) -> Option<&dyn super::traits::UsbDevice> {
        if index >= self.num_active_devices as u16 {
            return Option::None;
        }
        return Option::Some(&self.devices[index as usize]);
    }
    fn get_mut_device(&mut self, index: u16) -> Option<&mut dyn super::traits::UsbDevice> {
        if index >= self.num_active_devices as u16 {
            return Option::None;
        }
        return Option::Some(&mut self.devices[index as usize]);
    }
    fn install_interrupt_poller(
        &mut self,
        raw_device: &mut dyn super::traits::UsbDevice,
        interface_index: u8,
        endpoint_index: u8,
        interval: u8,
        report_address: u32,
        bytes_to_transfer: u16,
        callback: Option<super::traits::UsbInterruptPollerCallbackFn>,
    ) {
        let device = unsafe { &*(raw_device as *const dyn UsbDevice as *const () as *const OhciDevice) };
        let interface = &device.configuration.interfaces[interface_index as usize];
        let endpoint = &interface.endpoints[endpoint_index as usize];
        match &endpoint.real_endpoint {
            Unassigned => simple_kernel_panic(self.module.name(), "Unassigned Endpoint\n"),
            OhciGeneralEndpointRealEndpoint::Periodic(ep) => {
                if ep.get_endpoint_descriptor().is_set(F) {
                    simple_kernel_panic(self.module.name(), "install_interrupt_poller: endpoint_index points to a endpoint of a wrong type\n");
                } else {
                    let mut desc = ep.get_endpoint_descriptor();
                    let mut td = GeneralTD::new(ep.transfer_descriptors as *mut u32);
                    let td_offset = unsafe {(ep.transfer_descriptors as *const RawGeneralTD).offset_from_unsigned(self.transfer_descriptors.interrupt_tds.as_ptr()) };
                    let dir = match (endpoint.get_endpoint_address() >> 7) {
                        0 => Direction::Out.as_ohci(),
                        1 => Direction::In.as_ohci(),
                        _ => simple_kernel_panic("OhciEndpointDescriptor/from_general", "Just How?\n"),
                    };
                    td.zero_out();
                    td.set_part(GeneralTDPart::Dp, dir);
                    td.set_part(GeneralTDPart::T, 0b00);
                    // delays 1 frame so control interrupts come first
                    td.set_part(GeneralTDPart::Di, 0b001);
                    td.set(GeneralTDBitPart::R, true);
                    td.write_cbp(report_address);
                    td.write_buffer_end(report_address + (bytes_to_transfer - 1) as u32);
                    td.write_next_td(OHCI_TRANSFER_DESCRIPTOR_PROCESSED | ((td_offset & 0xFF) as u32) << 20 | (bytes_to_transfer as u32) << 4);

                    if let Option::Some(backcall) = callback {
                        let new_size = self.interrupt_pollers.len() + 1;
                        assert!(1024 > new_size);
                        resize_slice(&mut self.interrupt_pollers, new_size);
                        self.interrupt_pollers[new_size - 1] =
                            OhciInterruptPoller { callback: backcall , td_offset: td_offset as u8 };
                    }

                    desc.write_head_p(td.address());
                    desc.write_tail_p(OHCI_TRANSFER_DESCRIPTOR_PROCESSED | ((td_offset & 0xFF) as u32) << 20 | (bytes_to_transfer as u32) << 4);
                    desc.set(EndpointDescriptorBitPart::K, false);
                }
            }
            OhciGeneralEndpointRealEndpoint::NonPeriodic(_ep) => {
                simple_kernel_panic(self.module.name(), "install_interrupt_poller: endpoint_index points to a endpoint of a wrong type\n");
            }
        }
    }
    fn number_of_active_devices(&self) -> u16 {
        return self.num_active_devices as u16;
    }
    fn number_of_potential_devices(&self) -> u16 {
        return self.num_potential_devices as u16;
    }
    fn start(&mut self) {
        /*
         * NOTICE: Doesn´t activate the Controller.
         */
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Mie);
        self.bar.hc_control().enable_all_processing();
    }
    fn stop(&mut self) {
        /*
         * NOTICE: Doesn´t deactivate the Controller.
         */
        self.bar.hc_interrupt_disable().disable(OhciInterrupt::Mie);
        self.bar.hc_control().disable_all_processing();
    }
    fn untraited_work0(&mut self) -> Option<bool> {
        /* This is setting the addresses of the devices and prepares the port*/

        let mut tmp_buffer: [u32; 4] = [0; 4];

        if !self.bar.hc_rh_descriptor_a().is_set(Nps) {
            /* enables global power*/
            self.bar.hc_rh_status().set(Lpsc);
            sleep(10);
        }

        let mut current_address = 1;

        /* This sets up the OhciDevice and assignes a control ep to each device*/
        for i in 0..self.num_potential_devices {
            let mut status = self.bar.hc_rh_port_status(i as u32 + 1);
            if status.is_set(data_structures::OhciRhPortStatusBitPart::Ccs) {
                if !status.is_set(data_structures::OhciRhPortStatusBitPart::Pps){
                    /* Power down. Activate it
                     * Notice: If this port is controlled by global power. It´s allready enabled
                     */
                    status.set(OhciRhPortStatusBitPart::Pps);
                }
                status.set(Prs); /* Resets Port*/

                while status.is_set(Prs) {} /* Waiting for reset to be complete */

                if self.bar.hc_interrupt_status().is_set(OhciInterrupt::Rhsc) {
                    self.bar.hc_interrupt_status().set(OhciInterrupt::Rhsc);
                }
                self.devices[i as usize] = OhciDevice::new_resetted(i + 1, self.control_list.ep(i), unsafe { &*ptr::from_ref(&self.control_list) });

                let mut endpoint = OhciEndpointDescriptor::new(tmp_buffer.as_mut_ptr() as *mut c_void);
                endpoint.zero_out();

                if status.is_set(OhciRhPortStatusBitPart::Lsda) {
                    /* Endpoint is for a low speed device*/
                    endpoint.set(structures::endpoint::EndpointDescriptorBitPart::S, true);
                    info!(&mut self.module, "Port {} is connected to a low speed device\n", i + 1);
                }else {
                    info!(&mut self.module, "Port {} is connected to a full speed device\n", i + 1);
                }
                endpoint.set_part(Mps, 8); /* Minimum bytes supported are 8 bytes*/
                self.control_list.append_endpoint(endpoint);

                if i != 0 {
                    /* Disables execution of ep*/
                    self.control_list.ep(i - 1).set(EndpointDescriptorBitPart::K, true);
                }


                let device = &mut self.devices[i as usize];

                device.control_ep.set_address_and_length(
                    ptr::from_ref(&mut self.transfer_descriptors.control_tds[self.transfer_descriptors.allocated_control_tds as usize])
                        as *mut c_void,
                    4);
                device.set_address(current_address as u16);
                device.device_address = current_address as u8;
                device.state = UsbDeviceState::Address;
                device.control_ep.get_endpoint_descriptor().set_part(EndpointDescriptorPart::Fa, current_address as u32);
                current_address += 1;
                self.transfer_descriptors.allocated_control_tds += 4;
            }else {
                let device = &mut self.devices[i as usize];
                *device =
                    OhciDevice::new_detached(i + 1, self.control_list.ep(i), unsafe { &*ptr::from_ref(&self.control_list) });
                device.control_ep.set_address_and_length(
                    ptr::from_ref(&mut self.transfer_descriptors.control_tds[self.transfer_descriptors.allocated_control_tds as usize])
                        as *mut c_void,
                    4);
                let mut endpoint = OhciEndpointDescriptor::new(tmp_buffer.as_mut_ptr() as *mut c_void);
                endpoint.zero_out();
                endpoint.set_part(Mps, 8);
                endpoint.set(K, true);
                endpoint.set(EndpointDescriptorBitPart::Dum, true);
                self.control_list.append_endpoint(endpoint);
                self.transfer_descriptors.allocated_control_tds += 4;
            }
        }
        self.num_active_devices = current_address - 1;
        /* Only activates Rhcs after every port was reset, since a port reset causes a port update*/
        if self.bar.hc_interrupt_status().is_set(OhciInterrupt::Rhsc) {
            self.bar.hc_interrupt_status().set(OhciInterrupt::Rhsc);
        }
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Rhsc);
        Option::Some(true)
    }
    fn untraited_work1(&mut self) -> Option<bool> {
        None
    }
    fn untraited_work2(&mut self) -> Option<bool> {
        for device in &mut *self.devices {
            let mut tmp: [u32; 4] = [0;4];

            if let UsbDeviceState::Detached = device.state {
                continue;
            }
            for interface in &mut *device.configuration.interfaces {
                for endpoint in &mut *interface.endpoints {
                    match UsbTransferType::from_u8(endpoint.get_bm_attributes() & 3) {
                        UsbTransferType::Control => {
                            let dev_addr = device.device_address;
                            let real_point = unsafe { &mut *(&raw mut device.control_ep) };
                            endpoint.real_endpoint = OhciGeneralEndpointRealEndpoint::NonPeriodic(real_point);
                            warn!(&mut self.module, "device {} has an extra control endpoint. Routing it to default control endpoint...\n", dev_addr);
                        }
                        UsbTransferType::Isochronous => {
                            let mut epd = OhciEndpointDescriptor::new(tmp.as_mut_ptr() as *mut c_void);
                            epd.zero_out();
                            epd.set(EndpointDescriptorBitPart::F, true);
                            epd.from_general(device.device_address as u32, device.control_ep.get_endpoint_descriptor().is_set(EndpointDescriptorBitPart::S), endpoint);

                            let index = self.interrupt_list.install(1, epd);

                            let real_endpoint_ptr = &raw mut self.endpoints.periodic_endpoints[self.endpoints.allocated_periodic_endpoints as usize];
                            unsafe {
                                *real_endpoint_ptr = OhciPeriodicEndpoint::new(self.interrupt_list.ep(index as u8), UsbTransferType::Isochronous);
                                (*real_endpoint_ptr).set_address_and_length(
                                    ptr::from_mut(&mut self.transfer_descriptors.isochronous_tds[self.transfer_descriptors.allocated_isochronous_tds as usize]) as *mut c_void,
                                    1
                                );
                                (*real_endpoint_ptr).set_interval(1);
                                self.transfer_descriptors.allocated_isochronous_tds += 1;
                            }

                            endpoint.real_endpoint = OhciGeneralEndpointRealEndpoint::Periodic(unsafe {&mut *real_endpoint_ptr});
                            self.endpoints.allocated_periodic_endpoints += 1;
                        }
                        UsbTransferType::Bulk => {
                            let mut epd = OhciEndpointDescriptor::new(tmp.as_mut_ptr() as *mut c_void);
                            epd.zero_out(); // Sets F to 0
                            epd.from_general(device.device_address as u32, device.control_ep.get_endpoint_descriptor().is_set(EndpointDescriptorBitPart::S), endpoint);

                            let index = self.bulk_list.append_endpoint(epd);
                            let real_endpoint_ptr = &raw mut self.endpoints.non_periodic_endpoints[self.endpoints.allocated_non_periodic_endpoints as usize];

                            unsafe {
                                *real_endpoint_ptr = OhciNonPeriodicEndpoint::new(UsbTransferType::Bulk, self.bulk_list.ep(index));
                                (*real_endpoint_ptr).set_address_and_length(
                                    ptr::from_mut(&mut self.transfer_descriptors.bulk_tds[self.transfer_descriptors.allocated_bulk_tds as usize]) as *mut c_void,
                                    1
                                );
                                self.transfer_descriptors.allocated_bulk_tds += 1;
                            }

                            endpoint.real_endpoint = OhciGeneralEndpointRealEndpoint::NonPeriodic(unsafe { &mut *real_endpoint_ptr });
                            self.endpoints.allocated_non_periodic_endpoints += 1;
                        }
                        UsbTransferType::Interrupt => {
                            let mut epd = OhciEndpointDescriptor::new(tmp.as_mut_ptr() as *mut c_void);
                            epd.zero_out(); // Sets F to 0
                            epd.from_general(device.device_address as u32, device.control_ep.get_endpoint_descriptor().is_set(EndpointDescriptorBitPart::S), endpoint);

                            let mut highest_val = 0;
                            for i in (0usize..=7usize).rev() {
                                if endpoint.b_interval & (1 << i) != 0 {
                                    highest_val = 1 << i;
                                    break;
                                }
                            }
                            let pow2_aligned_interval = endpoint.b_interval & !(highest_val - 1);
                            let index = self.interrupt_list.install(pow2_aligned_interval, epd);

                            let real_endpoint_ptr = &raw mut self.endpoints.periodic_endpoints[self.endpoints.allocated_periodic_endpoints as usize];
                            unsafe {
                                *real_endpoint_ptr = OhciPeriodicEndpoint::new(self.interrupt_list.ep(index as u8), UsbTransferType::Interrupt);
                                (*real_endpoint_ptr).set_address_and_length(
                                    ptr::from_mut(&mut self.transfer_descriptors.interrupt_tds[self.transfer_descriptors.allocated_interrupt_tds as usize]) as *mut c_void,
                                    1
                                );
                                (*real_endpoint_ptr).set_interval(pow2_aligned_interval);
                                self.transfer_descriptors.allocated_interrupt_tds += 1;
                            }

                            endpoint.real_endpoint = OhciGeneralEndpointRealEndpoint::Periodic(unsafe {&mut *real_endpoint_ptr});
                            self.endpoints.allocated_periodic_endpoints += 1;
                        }
                    }
                }
            }
        }
        self.bar.hc_control().set(data_structures::OhciControlBitPart::Ple, true);
        Option::Some(true)
    }
}
/*
 * In the far Future: Look at Section 5.2.10 and skip a pipe, if the bus is overused
 */
impl OhciController {
    /**
     * No SMM or BIOS driver, since it´s deactivated by ExitBootServices of the Bootloader
     * Notice: Full Speed MaxPacketSize = 8,16,32 or 64 bytes
     *         Low Speed MaxPacketSize = 8
     */
    pub fn initalize(&mut self) {
        info!(self.module, "Revision 0x{:x}\n", self.bar.hc_revision());

        let new_hcca_addr = match self.private_physical_allocator.alloc_zero(1) {
            Ok(mb) => mb.base,
            Err(_e) => simple_kernel_panic(self.module.name(), "Could not allocate memory for Hcca\n"),
        };
        self.reset();
        self.bar.write_hc_hcca(0xFFFFFFFF);
        if (!self.bar.hc_hcca()) + 1 > 0x1000 {
            simple_kernel_panic(self.module.name(), "Hcca alignment is greather than 0x1000\n")
        }
        self.bar.write_hc_hcca(new_hcca_addr as u32);
        self.hcca = OhciHcca::new(new_hcca_addr as *mut c_void);
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Mie);
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Oc);
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Ue);
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Rd);
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Wdh);

        // Total Timespan from SOF to SOF is 1ms
        self.bar.write_hc_periodic_start(0x2A2F); // Interrupt/Isochronous List is preferred after ~900 microseconds
        self.bar.write_hc_ls_threshold(0x500); // 1280 bit times. 1 bit time = 83,33ns (or 1 cycle from 12 Mhz clock)
        /*
         * FSMPS = floor( (FrameInterval - 210) * 6/7 )
         */
        self.bar
            .hc_fm_interval()
            .set_part(data_structures::OhciFmIntervalPart::Fsmps, 0x2778);

        self.interrupt_list = OhciHccaInterruptList::new(new_hcca_addr as *mut u32);
        self.interrupt_list.initialize(&mut self.private_physical_allocator);

        self.bulk_list = unsafe { OhciNonPeriodicList::new(self.bar.address().add(11), self.bar.address().add(10), self.bar.address().add(2), 2) };
        self.control_list = unsafe { OhciNonPeriodicList::new(self.bar.address().add(9), self.bar.address().add(8), self.bar.address().add(2), 1 ) };

        self.bulk_list.initialize(&mut self.private_physical_allocator);
        self.control_list.initialize(&mut self.private_physical_allocator);

        self.num_potential_devices =
            self.bar
                .hc_rh_descriptor_a()
                .get(data_structures::OhciRhDescriptorAPart::Ndp) as u8;
        info!(&mut self.module, "Potential Devices {}\n", self.num_potential_devices);

        self.device_memory = alloc_zero_or_crash(&mut self.private_physical_allocator, 1, &mut self.module, "Could not allocate device Array\n");

        self.interfaces = PageAllocator::new(&mut self.private_physical_allocator, 32);

        self.transfer_descriptors = OhciControllerTds::new(&mut self.module, &mut self.private_physical_allocator);
        self.endpoints = OhciControllerEndpoints::new(&mut self.module, &mut self.private_physical_allocator);

        self.hid_memory = alloc_zero_or_crash(&mut self.private_physical_allocator, 1, &mut self.module, "Could not allocate HID Array\n");

        self.interrupt_pollers_memory = alloc_zero_or_crash(&mut self.private_physical_allocator, 4, &mut self.module, "Could not allocate interrupt poller Array\n");

        self.interrupt_pollers = unsafe { slice::from_raw_parts_mut(self.interrupt_pollers_memory.as_mut_ptr(), 0) };

        self.hid_descriptors = unsafe { slice::from_raw_parts_mut(self.hid_memory.as_mut_ptr(), 0x1000 / size_of::<UsbHID>()) };
        self.devices = unsafe { slice::from_raw_parts_mut(self.device_memory.as_mut_ptr(), self.num_potential_devices as usize) };
        self.bar.hc_interrupt_enable().enable(OhciInterrupt::Rd);
        self.bar.hc_control().set(data_structures::OhciControlBitPart::Cle, true);
        self.bar.hc_control().set_part(data_structures::OhciControlPart::Hcfs, HostControllerFunctionalState::UsbResume as u32);
        self.bar.hc_control().set_part(data_structures::OhciControlPart::Hcfs, HostControllerFunctionalState::UsbOperational as u32);
    }
    /**
     * Causes
     *  1:1 Control/Bulk Servicing Ratio
     *  Isochronous, Periodic, Bulk, Control, Interrupt Lists to be disabled
     *  HCFS to be set to 'UsbReset'
     *  Control List Filled to be set to 0
     */
    pub fn reset(&mut self) {
        self.bar
            .hc_command_status()
            .set(OhciCommandStatusBitPart::Hcr);
        sleep(10);

        while self
            .bar
            .hc_command_status()
            .is_set(OhciCommandStatusBitPart::Hcr)
        { /* Wait*/ }
    }
}

pub fn create_ohci(
    pci_bus: &PciBus,
    pci_device: u64,
    physical_allocator: &mut Allocator,
    isr_vector: u8,
) {
    unsafe {
        let ohci_controller = &raw mut OHCI_CONTROLLER;
        (*ohci_controller).initialize_controller(
            pci_bus,
            physical_allocator,
            isr_vector,
            pci_device,
        );
        (*ohci_controller).untraited_work0();
        (*ohci_controller).gather_device_information();
        (*ohci_controller).configure_devices();
        (*ohci_controller).untraited_work2();
    }
}
