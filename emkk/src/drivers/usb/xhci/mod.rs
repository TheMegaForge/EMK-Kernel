use core::{
    arch::asm,
    ffi::c_void,
    net::IpAddr::V6,
    ptr::{dangling, null_mut, slice_from_raw_parts},
    slice,
};

pub mod configuration_parser;
pub mod data_structures;
pub mod registers;
pub mod request_impl;
pub mod structures;
use crate::{
    aml::definitions::TermArgInt::Mod,
    arch::{isr::ISRRegisters, lapic::LocalApic},
    downcast, downcast_mut,
    drivers::usb::{
        independent::{Direction, UsbDeviceInformation, UsbProtocol, UsbSpeed, UsbTransferType},
        standard_requests::{UsbDeviceStandardRequest, UsbHID},
        traits::{
            UsbConfiguration, UsbController, UsbDevice, UsbEndpoint, UsbInterruptPollerCallbackFn,
        },
        xhci::{
            self,
            configuration_parser::XhciDeviceConfiguration,
            data_structures::{
                XhciExtendedCapability, XhciProtocolDefinitionPart, XhciSupportedProtocol,
                XhciTrbId,
            },
            registers::{
                XhciBar, XhciHccParams1BitPart, XhciHccParams1Part, XhciHcsParams1Part,
                XhciPortScBitPart, XhciPortScPart, XhciUsbCmdBitPart, XhciUsbStsBitPart,
            },
            structures::{
                RawXhciNormalTrb, RawXhciTrb, XHCI_SLOT_TYPE_GENERAL,
                XhciCommandCompletionEventTrb, XhciLinkTrb, XhciNormalTrbBitPart,
                XhciTransferEventTrb,
                command_ring::XhciCommandRing,
                contexts::{XhciEndpointContext32, XhciInputContext32},
                device::XhciDevice,
                endpoint::{XhciEndpoint, XhciEndpointDescriptor},
                interface::XhciInterface,
                interrupter::XhciInterrupter,
            },
        },
    },
    fixed_vaddrs::ref_processor_mut,
    hal::{
        memory::allocator::Allocator,
        pci_bus::{PciBarIndex, PciBus},
        print::{Module, simple_kernel_panic},
    },
    info,
    utils::{
        allocators::PageAllocator,
        memory::{alloc_zero_or_crash, free_or_crash, memcpy},
        slices::{invalid_mut_slice, resize_slice},
        traits::AsU64,
    },
};

type XhciEventCallback = *mut dyn FnMut(*mut RawXhciTrb, XhciTrbId);

pub struct XhciControllerInterfaceData {
    hids: &'static mut [UsbHID],
    interfaces: PageAllocator<XhciInterface>,
    general_endpoints: PageAllocator<XhciEndpointDescriptor>,
    supported_protocols: &'static mut [XhciSupportedProtocol],
    devices: &'static mut [XhciDevice],
    endpoints: &'static mut [XhciEndpoint],
    active_devices: u32,
    current_device_context: *mut c_void,
    current_device_context_index: u8,
    current_input_context: *mut c_void,
    current_input_context_index: u8,
    current_endpoint_index: u8,
    current_hid_index: u8,
}

impl XhciControllerInterfaceData {
    pub const fn empty() -> Self {
        return Self {
            hids: invalid_mut_slice(),
            interfaces: PageAllocator::empty(),
            general_endpoints: PageAllocator::empty(),
            supported_protocols: invalid_mut_slice(),
            devices: invalid_mut_slice(),
            endpoints: invalid_mut_slice(),
            active_devices: 0,
            current_device_context: null_mut(),
            current_device_context_index: 0,
            current_input_context: null_mut(),
            current_input_context_index: 0,
            current_endpoint_index: 0,
            current_hid_index: 0,
        };
    }
    pub fn initialize(&mut self, allocator: &mut Allocator, module: &mut Module<'static>) {
        let device_memory =
            alloc_zero_or_crash(allocator, 1, module, "Could not allocate device Array\n");
        let endpoint_memory =
            alloc_zero_or_crash(allocator, 1, module, "Could not allocate endpoint Array\n");
        let hid_memoy = alloc_zero_or_crash(allocator, 1, module, "Could not allocate HID Array\n");
        self.hids = hid_memoy.as_mut_slice(0);
        self.interfaces = PageAllocator::new(allocator, 128);
        self.general_endpoints = PageAllocator::new(allocator, 128);
        self.devices = device_memory.as_mut_slice(0);
        self.endpoints = endpoint_memory.as_mut_slice(0);
        self.current_device_context =
            alloc_zero_or_crash(allocator, 1, module, "Could not allocate device Contexts\n")
                .as_mut_ptr();
        self.current_input_context =
            alloc_zero_or_crash(allocator, 1, module, "Could not allocate input Contexts\n")
                .as_mut_ptr();
    }

    pub fn request_device_context(&mut self, module: &mut Module<'static>) -> u64 {
        if self.current_device_context_index == 3 {
            self.current_device_context = alloc_zero_or_crash(
                unsafe { &mut *XHCI_TEMPORARY_ALLOCATOR },
                1,
                module,
                "Could not allocate new device contexts\n",
            )
            .as_mut_ptr();
            self.current_device_context_index = 0;
        }
        let ret = unsafe {
            self.current_device_context
                .add(0x400 * self.current_device_context_index as usize)
        };
        self.current_device_context_index += 1;
        return ret as u64;
    }
    pub fn request_input_context(
        &mut self,
        module: &mut Module<'static>,
    ) -> &'static mut XhciInputContext32 {
        if self.current_input_context_index == 2 {
            self.current_input_context = alloc_zero_or_crash(
                unsafe { &mut *XHCI_TEMPORARY_ALLOCATOR },
                1,
                module,
                "Could not allocate new device contexts\n",
            )
            .as_mut_ptr();
            self.current_input_context_index = 0;
        }
        let ret = unsafe {
            self.current_input_context
                .add(0x420 * self.current_input_context_index as usize)
        };
        self.current_input_context_index += 1;
        return unsafe { &mut *(ret as *mut XhciInputContext32) };
    }

    pub fn request_device(&mut self) -> &'static mut XhciDevice {
        let ret = unsafe { &mut *(&raw mut self.devices[self.active_devices as usize]) };
        self.active_devices += 1;
        return ret;
    }
    pub fn request_endpoint(&mut self) -> &'static mut XhciEndpoint {
        let ret = unsafe { &mut *(&raw mut self.endpoints[self.current_endpoint_index as usize]) };
        self.current_endpoint_index += 1;
        return ret;
    }
}

pub struct XhciInterrupPoller {
    r#fn: UsbInterruptPollerCallbackFn,
    dev_addr: u8,
    epid: u8,
}

pub struct XhciController {
    module: Module<'static>,
    present: bool,
    bar: XhciBar,
    dbesl: u8,
    dbesld: u8,
    dcbaap: &'static mut [u64],
    command_ring: XhciCommandRing,
    interrupter: XhciInterrupter,
    event_callback_fn: Option<XhciEventCallback>,
    interface_data: XhciControllerInterfaceData,
    private_memory: Allocator,
    pollers: PageAllocator<XhciInterrupPoller>,
}

impl XhciController {
    pub const fn not_present() -> Self {
        return Self {
            module: Module::new("Xhci"),
            present: false,
            bar: XhciBar::new(null_mut()),
            dbesl: 0,
            dbesld: 0,
            dcbaap: invalid_mut_slice(),
            command_ring: XhciCommandRing::empty(),
            interrupter: XhciInterrupter::empty(),
            event_callback_fn: Option::None,
            interface_data: XhciControllerInterfaceData::empty(),
            private_memory: Allocator::empty(),
            pollers: PageAllocator::empty(),
        };
    }

    pub fn get_speed_for_port(&self, port_index: u8) -> Option<UsbSpeed> {
        let port_speed = self.bar.portsc(port_index).get(XhciPortScPart::PortSpeed);
        for protocol in &*self.interface_data.supported_protocols {
            if port_index >= protocol.compatible_port_offset
                && protocol.compatible_port_offset + protocol.compatible_port_count > port_index
            {
                if protocol.psic == 0 {
                    return Option::Some(match port_speed {
                        1 => UsbSpeed::FullSpeed,
                        2 => UsbSpeed::LowSpeed,
                        3 => UsbSpeed::HighSpeed,
                        4 => UsbSpeed::SuperSpeedGen1x1,
                        5 => UsbSpeed::SuperSpeedPlusGen2x1,
                        6 => UsbSpeed::SuperSpeedPlusGen1x2,
                        7 => UsbSpeed::SuperSpeedPlusGen2x2,
                        _ => simple_kernel_panic(
                            self.module.name(),
                            "How? - Invalid Port Speed value of a Protocol, while PSIC = 0\n",
                        ),
                    });
                } else {
                    for i in 0..protocol.psic + 1 {
                        let definition = protocol.get_definition(i);
                        if port_speed
                            == definition.get(data_structures::XhciProtocolDefinitionPart::Psiv)
                        {
                            let plt = definition.get(XhciProtocolDefinitionPart::Plt);
                            let pfd = definition.pfd() as u32;
                            let psie = definition.get(XhciProtocolDefinitionPart::Psie);
                            let psim = definition.get(XhciProtocolDefinitionPart::Psim);
                            return Option::Some(match (plt, pfd, psie, psim) {
                                (0, 0, 2, 12) => UsbSpeed::FullSpeed,
                                (0, 0, 1, 1500) => UsbSpeed::LowSpeed,
                                (0, 0, 2, 480) => UsbSpeed::HighSpeed,
                                (0, 1, 3, 5) => UsbSpeed::SuperSpeedGen1x1,
                                (0, 1, 3, 10) => {
                                    if protocol.revision_minor == 0x10 {
                                        UsbSpeed::SuperSpeedPlusGen2x1
                                    } else {
                                        UsbSpeed::SuperSpeedPlusGen1x2
                                    }
                                }
                                (0, 1, 3, 20) => UsbSpeed::SuperSpeedPlusGen2x2,
                                _ => simple_kernel_panic(
                                    self.module.name(),
                                    "How? - Unknown Protocol definition\n",
                                ),
                            });
                        }
                    }
                }
            }
        }
        return Option::None;
    }
    pub fn get_protocol_for_port(&self, port_index: u8) -> Option<UsbProtocol> {
        let port_speed = self.bar.portsc(port_index).get(XhciPortScPart::PortSpeed);
        for protocol in &*self.interface_data.supported_protocols {
            if port_index >= protocol.compatible_port_offset
                && protocol.compatible_port_offset + protocol.compatible_port_count > port_index
            {
                return Option::Some(match (protocol.revision_major, protocol.revision_minor) {
                    (2, 0) => UsbProtocol::Usb2,
                    (3, 0) => UsbProtocol::Usb3,
                    (3, 0x10) => UsbProtocol::Usb3_1,
                    (3, 0x20) => UsbProtocol::Usb3_2,
                    _ => simple_kernel_panic(
                        self.module.name(),
                        "How? - Unknown Revision of a Protocol\n",
                    ),
                });
            }
        }
        return Option::None;
    }

    pub fn initialize(&mut self, allocator: &mut Allocator) {
        self.stop();
        self.bar.usbcmd().set(XhciUsbCmdBitPart::HcRst, true);
        while self.bar.usbcmd().is_set(XhciUsbCmdBitPart::HcRst) {}
        let dcbaap_mb = alloc_zero_or_crash(
            allocator,
            1,
            &mut self.module,
            "Could not allocate Device Context Structure Pointer Array\n",
        );
        self.dcbaap = unsafe {
            slice::from_raw_parts_mut(
                dcbaap_mb.as_mut_ptr(),
                self.bar.hcsparams1().get(XhciHcsParams1Part::MaxSlots) as usize,
            )
        };
        let cmd_ring_addr =
            alloc_zero_or_crash(allocator, 2, &mut self.module, "Could not allocate \n").base;
        self.bar.crcr().write_command_ring_pointer(cmd_ring_addr);
        self.command_ring =
            XhciCommandRing::new(self.bar.doorbell(0), self.bar.crcr(), cmd_ring_addr);
        if self.bar.hcsparams2().max_scratchpad_bufs() != 0 {
            let bytes_needed = self.bar.hcsparams2().max_scratchpad_bufs() * 8;
            let mut pages_needed = bytes_needed / 0x1000;
            if bytes_needed % 0x1000 != 0 {
                pages_needed += 1;
            }
            let array_mb = alloc_zero_or_crash(
                allocator,
                pages_needed,
                &mut self.module,
                "Could not allocate Scratchpad Buffer Array\n",
            );
            for i in 0..self.bar.hcsparams2().max_scratchpad_bufs() {
                unsafe {
                    *array_mb.as_mut_ptr::<u64>().add(i as usize) = alloc_zero_or_crash(
                        allocator,
                        1,
                        &mut self.module,
                        "Could not allocate Scratchpad Buffer\n",
                    )
                    .base;
                }
            }
            self.dcbaap[0] = array_mb.base;
        }

        self.bar.write_dcbaap(self.dcbaap.as_ptr().addr() as u64);
        let mut cmd = self.bar.usbcmd();
        cmd.set(XhciUsbCmdBitPart::Inte, true);
        cmd.set(XhciUsbCmdBitPart::Hsee, true);
        let raw_x_ecp = self.bar.hccparams1().get(XhciHccParams1Part::XEcp);
        if raw_x_ecp == 0 {
            simple_kernel_panic(
                self.module.name(),
                "Does not support extended capabilities\n",
            );
        }
        let mut x_ec = unsafe {
            &*(self.bar.get_base().add((raw_x_ecp as usize) << 2) as *const XhciExtendedCapability)
        };

        let supported_protocol_capability_array_mb = alloc_zero_or_crash(
            allocator,
            1,
            &mut self.module,
            "Could not allocate supported Protocol Capability Array\n",
        );
        let mut num_present_protocols = 0;
        loop {
            if x_ec.capability_id == 2 {
                let supported_protocol = unsafe {
                    &mut *supported_protocol_capability_array_mb
                        .as_mut_ptr::<XhciSupportedProtocol>()
                        .add(num_present_protocols as usize)
                };
                let ptr = (&raw const *x_ec) as *const u32;
                let dword0 = unsafe { *ptr };
                let dword1 = unsafe { *ptr.add(1) };
                supported_protocol.name_string = [
                    (dword1 & 0xFF) as u8,
                    ((dword1 >> 8) & 0xFF) as u8,
                    ((dword1 >> 16) & 0xFF) as u8,
                    ((dword1 >> 24) & 0xFF) as u8,
                ];
                let dword2 = unsafe { *ptr.add(2) };
                let dword3 = unsafe { *ptr.add(3) };
                supported_protocol.revision_minor = ((dword0 >> 16) & 0xFF) as u8;
                supported_protocol.revision_major = ((dword0 >> 24) & 0xFF) as u8;
                supported_protocol.compatible_port_offset = (dword2 & 0xFF) as u8;
                supported_protocol.compatible_port_count = ((dword2 >> 8) & 0xFF) as u8;
                supported_protocol.protcol_defined = ((dword2 >> 16) & 0xFFF) as u16;
                supported_protocol.psic = ((dword2 >> 28) & 0xF) as u8;
                supported_protocol.protocol_slot_type = (dword3 & 0x1F) as u8;
                supported_protocol.definition_ptr = unsafe { ptr.add(4) };
                num_present_protocols += 1;
            }
            let next_ptr = unsafe { *((&raw const x_ec.capability_id) as *const u32) >> 8 } & 0xFF;
            if next_ptr == 0 {
                break;
            }
            x_ec = unsafe {
                &*(((&raw const *x_ec) as *const c_void).add((next_ptr as usize) << 2)
                    as *const XhciExtendedCapability)
            }
        }
        self.interface_data.supported_protocols = unsafe {
            slice::from_raw_parts_mut(
                supported_protocol_capability_array_mb.as_mut_ptr(),
                num_present_protocols,
            )
        };
        self.interrupter = XhciInterrupter::new(self.bar.ir(0), allocator);
        self.bar
            .config()
            .set_max_slots_en(self.bar.hcsparams1().get(XhciHcsParams1Part::MaxSlots) as u8);
        self.start();
    }
}

pub static mut XHCI_CONTROLLER: XhciController = XhciController::not_present();
fn xhci_interrupt0(_: &ISRRegisters) {
    #[allow(static_mut_refs)]
     unsafe { &mut XHCI_CONTROLLER }
        .interrupter
        .consume_events(|trb, trb_type| {
            #[allow(static_mut_refs)]
            let xhci_controller = unsafe { &mut XHCI_CONTROLLER };
            match trb_type {
            XhciTrbId::PortStatusChangeEvent | XhciTrbId::CommandCompletionEvent => {
                match xhci_controller.event_callback_fn {
                    Some(callback_fn) => {
                        let r#fn = unsafe { &mut *callback_fn };
                        (r#fn)(trb, trb_type);
                    }
                    None => {}
                }
            }
            XhciTrbId::TransferEvent => {
                let transfer_event = XhciTransferEventTrb::from(trb);
                if transfer_event.completion_code() != 1 {
                    simple_kernel_panic(xhci_controller.module.name(), "Transfer failed\n");
                }
                let epid = transfer_event.endpoint_id();

                if epid == 1 {
                    for device in &mut *xhci_controller.interface_data.devices {
                        if device.slot_id == transfer_event.slot_id() as u16 {
                            device.signaled = true;
                            break;
                        }
                    }
                } else {
                    let mut found = false;
                    for device in &mut *xhci_controller.interface_data.devices {
                        let this_dev_addr = device.device_address();
                        for interface in &mut *device.configuration.interfaces {
                            for endpoint in &mut *interface.endpoints {
                                let this_epid;
                                if let Direction::In = endpoint.get_direction() {
                                    this_epid = endpoint.endpoint_number() + 2;
                                } else {
                                    this_epid = endpoint.endpoint_number() + 1;
                                }
                                if epid == this_epid {
                                    match &mut endpoint.endpoint {
                                        structures::endpoint::XhciEndpointDescriptorRealEndpoint::Open(ep) => {
                                            let next_type = unsafe { (ep.enqueue_pointer.add(7).read_volatile() >> 10) & 0x3F};
                                            if next_type == XhciTrbId::Link as u32{
                                                let normal_trb = RawXhciNormalTrb::from_mut(ep.enqueue_pointer);
                                                normal_trb.set(structures::XhciNormalTrbBitPart::C, !normal_trb.is_set(structures::XhciNormalTrbBitPart::C));
                                                let mut link_trb = XhciLinkTrb::from(unsafe { ep.enqueue_pointer.add(4) });
                                                ep.enqueue_pointer = link_trb.ring_segment_pointer() as *mut u32;
                                                link_trb.set(structures::XhciLinkTrbBitPart::C, !link_trb.is_set(structures::XhciLinkTrbBitPart::C));
                                            }else {
                                                let normal_trb = RawXhciNormalTrb::from_mut(ep.enqueue_pointer);
                                                normal_trb.set(structures::XhciNormalTrbBitPart::C, !normal_trb.is_set(structures::XhciNormalTrbBitPart::C));
                                                ep.enqueue_pointer = unsafe { ep.enqueue_pointer.add(4) };
                                            }
                                            xhci_controller.pollers.for_each(|_, raw_poller| {
                                                let poller = unsafe { &*raw_poller };
                                                if poller.epid == this_epid && poller.dev_addr == this_dev_addr {
                                                    (poller.r#fn)(unsafe { &XHCI_CONTROLLER });
                                                    return false;
                                                }
                                                true
                                            });
                                        }
                                        _ => simple_kernel_panic(xhci_controller.module.name(), "Invalid Endpoint State\n")
                                    }
                                    found = true;
                                    break;
                                }
                            }
                            if found {
                                break;
                            }
                        }
                        if found {
                            break;
                        }
                    }
                    if !found {
                        simple_kernel_panic(xhci_controller.module.name(), "Could not find Endpoint for the corrosponding transfer event\n")
                    }
                }
            }
            _ => todo!(),
        }});
    #[allow(static_mut_refs)]
    let xhci_controller = unsafe { &mut XHCI_CONTROLLER };
    xhci_controller.interrupter.clear_ip();
    xhci_controller.interrupter.clear_ehb();
    LocalApic::from_local_core().send_eoi();
}
static mut XHCI_TEMPORARY_ALLOCATOR: *mut Allocator = null_mut();
impl UsbController for XhciController {
    fn configure_devices(&mut self) -> bool {
        for i in 0..self.interface_data.active_devices as usize {
            let speed = self
                .get_speed_for_port(self.interface_data.devices[i].get_port())
                .unwrap();
            let device = &mut self.interface_data.devices.split_at_mut(i).1[0];
            let total_length;
            {
                let raw_descriptor = device.get_descriptor(
                    crate::drivers::usb::independent::UsbDescriptorType::Configuration,
                    0,
                    Option::None,
                    9,
                );
                let configuration_descriptor = raw_descriptor.as_configuration_descriptor();
                device.set_configuration(configuration_descriptor.b_configuration_value);
                match speed {
                    UsbSpeed::HighSpeed | UsbSpeed::LowSpeed | UsbSpeed::FullSpeed => {
                        device.device_information.max_power_ma =
                            configuration_descriptor.b_max_power as u16 * 2
                    }
                    UsbSpeed::SuperSpeedGen1x1
                    | UsbSpeed::SuperSpeedPlusGen1x2
                    | UsbSpeed::SuperSpeedPlusGen2x1
                    | UsbSpeed::SuperSpeedPlusGen2x2 => {
                        device.device_information.max_power_ma =
                            configuration_descriptor.b_max_power as u16 * 8
                    }
                }
                device.device_information.num_interfaces =
                    configuration_descriptor.b_num_interfaces;
                total_length = configuration_descriptor.w_total_length;
                free_or_crash(
                    &mut self.private_memory,
                    &raw_descriptor.data,
                    &mut self.module,
                    "Could not free Configuration Descriptor\n",
                );
            }
            {
                let raw_descriptor = device.get_descriptor(
                    crate::drivers::usb::independent::UsbDescriptorType::Configuration,
                    0,
                    Option::None,
                    total_length,
                );

                let length = self.interface_data.hids.len() as u16
                    - self.interface_data.current_hid_index as u16;

                let hid_descriptors = unsafe {
                    slice::from_raw_parts_mut(
                        &raw mut self.interface_data.hids
                            [self.interface_data.current_hid_index as usize],
                        length as usize,
                    )
                };
                device.configuration = XhciDeviceConfiguration::new(
                    hid_descriptors,
                    &mut self.interface_data.interfaces,
                    &mut self.interface_data.general_endpoints,
                    unsafe { raw_descriptor.data.as_ptr::<c_void>().add(9) },
                    device.device_information.num_interfaces as u16,
                );
                self.interface_data.current_hid_index +=
                    device.configuration.get_hid_interface_count();
                free_or_crash(
                    &mut self.private_memory,
                    &raw_descriptor.data,
                    &mut self.module,
                    "Could not free Configuration Descriptor\n",
                );
            }
        }
        return true;
    }
    fn error_present(&self) -> bool {
        return self.bar.usbsts().error_present();
    }
    fn gather_device_information(&mut self) -> bool {
        for i in 0..self.interface_data.active_devices as usize {
            let port = self.interface_data.devices[i].get_port();
            let protocol = self.get_protocol_for_port(port).unwrap();
            let device = &mut self.interface_data.devices.split_at_mut(i).1[0];
            {
                let raw_descriptor = device.get_descriptor(
                    crate::drivers::usb::independent::UsbDescriptorType::Device,
                    0,
                    Option::None,
                    8,
                );
                let dev_descriptor = raw_descriptor.as_device_descriptor();
                match protocol {
                    UsbProtocol::Usb3 | UsbProtocol::Usb3_1 | UsbProtocol::Usb3_2 => {
                        device.control_endpoint.update_max_packet_size(
                            2u16.pow(dev_descriptor.b_max_packet_size0 as u32) as u16,
                        )
                    }
                    UsbProtocol::Usb2 => device
                        .control_endpoint
                        .update_max_packet_size(dev_descriptor.b_max_packet_size0 as u16),
                    UsbProtocol::Usb1 => simple_kernel_panic(
                        self.module.name(),
                        "Usb 1 Protocol devices aren´t supported\n",
                    ),
                }
                free_or_crash(
                    &mut self.private_memory,
                    &raw_descriptor.data,
                    &mut self.module,
                    "Could not free device descriptor\n",
                );
                unsafe {
                    memcpy(
                        (&raw mut device.input_context.default_control_endpoint) as *mut c_void,
                        device.get_device_context(1) as *const c_void,
                        size_of::<XhciEndpointContext32>() as u32,
                    )
                };
                device.input_context.default_control_endpoint.set_part(
                    structures::contexts::XhciEndpointContext32Part::MaxPacketSize,
                    device.control_endpoint.get_maximum_packet_size() as u32,
                );
                device.input_context.input_context_configuration.a_line = 2;
                self.command_ring
                    .process(structures::XhciCommand::EvaluateContext {
                        slot_id: device.slot_id as u8,
                        input_context_address: (&raw const *device.input_context) as u64,
                    });
                self.interrupter.wait_for_events_or_crash(
                    self.module.name(),
                    "Timeout while waiting for EvaluateContext\n",
                );
            }
            {
                let raw_descriptor = device.get_descriptor(
                    crate::drivers::usb::independent::UsbDescriptorType::Device,
                    0,
                    Option::None,
                    18,
                );
                let dev_descriptor = raw_descriptor.as_device_descriptor();
                device.device_information =
                    UsbDeviceInformation::from_descriptor(raw_descriptor.as_device_descriptor());
                info!(
                    &mut self.module,
                    "Device {}: class {} subclass {}\n",
                    device.device_address(),
                    device.device_information.device_class,
                    device.device_information.device_sub_class
                );
                free_or_crash(
                    &mut self.private_memory,
                    &raw_descriptor.data,
                    &mut self.module,
                    "Could not free device descriptor\n",
                );

                device.device_information =
                    UsbDeviceInformation::from_descriptor(raw_descriptor.as_device_descriptor());
            }
        }
        return true;
    }
    fn get_device(&self, index: u16) -> Option<&dyn super::traits::UsbDevice> {
        if index >= self.interface_data.active_devices as u16 {
            return Option::None;
        }
        return Option::Some(&self.interface_data.devices[index as usize]);
    }
    fn get_mut_device(&mut self, index: u16) -> Option<&mut dyn super::traits::UsbDevice> {
        if index >= self.interface_data.active_devices as u16 {
            return Option::None;
        }
        return Option::Some(&mut self.interface_data.devices[index as usize]);
    }
    fn identity(&self) -> super::independent::UsbControllerType {
        return super::independent::UsbControllerType::XHC;
    }
    fn initialize_controller(
        &mut self,
        pci_bus: &PciBus,
        allocator: &mut Allocator,
        isr_vector: u8,
        pci_device: u64,
    ) -> bool {
        ref_processor_mut().install_isr(xhci_interrupt0, isr_vector);
        let pci0 = pci_bus.get_bar(pci_device, PciBarIndex::Index0).unwrap();
        pci0.map(ref_processor_mut().ref_mut_pager(), allocator);
        self.private_memory = allocator.subdivide(32); /* 131.072 bytes*/
        self.bar = XhciBar::new(pci0.get_address() as *mut c_void);
        self.bar.set_operational_base();
        self.bar.set_runtime_base();

        if !self.bar.hccparams1().is_set(XhciHccParams1BitPart::Ac64) {
            simple_kernel_panic(
                self.module.name(),
                "Xhci Controller does not support address of width 64\n",
            )
        }
        if self.bar.hccparams1().is_set(XhciHccParams1BitPart::Csz) {
            simple_kernel_panic(
                self.module.name(),
                "Xhci Controller doesn´t support 32 byte context data structures\n",
            );
        }

        if !self.bar.is_page_size_valid(0x1000) {
            simple_kernel_panic(
                self.module.name(),
                "Xhci Controller does not support a page size of 4k\n",
            )
        }
        self.pollers = PageAllocator::new(allocator, 128);
        match pci_bus.read_configuration_space_u32(pci_device, 0x60) {
            0x30 => info!(&mut self.module, "3.0 Compliant\n"),
            0x31 => info!(&mut self.module, "3.1 Compliant\n"),
            0x32 => info!(&mut self.module, "3.2 Compliant\n"),
            _ => simple_kernel_panic(self.module.name(), "Invalid SBRN value\n"),
        }
        {
            let tmp = pci_bus.read_configuration_space_u8(pci_device, 0x62);
            self.dbesl = tmp & 0xF;
            self.dbesld = (tmp >> 4) & 0xF;
        }
        self.interface_data.initialize(allocator, &mut self.module);
        self.initialize(allocator);
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
        let xhci_device = downcast_mut!(device, UsbDevice, XhciDevice);
        let config = xhci_device.get_configuration(0).unwrap();
        let interface = match config.get_interface(interface_index) {
            Some(interface) => interface,
            None => simple_kernel_panic(
                "XhciController/install_interrupt_poller",
                "Invalid interface_index\n",
            ),
        };
        let raw_endpoint = match interface.get_endpoint(endpoint_index as u16) {
            Some(endpoint) => endpoint,
            None => simple_kernel_panic(
                "XhciController/install_interrupt_poller",
                "Invalid endpoint_index\n",
            ),
        };
        let endpoint = downcast!(raw_endpoint, UsbEndpoint, XhciEndpointDescriptor);
        match &endpoint.endpoint {
            structures::endpoint::XhciEndpointDescriptorRealEndpoint::Unassigned => {
                simple_kernel_panic(self.module.name(), "Endpoint is not registered\n")
            }
            structures::endpoint::XhciEndpointDescriptorRealEndpoint::Open(endpoint) => {
                simple_kernel_panic(self.module.name(), "trying to open already open endpoint\n")
            }
            structures::endpoint::XhciEndpointDescriptorRealEndpoint::Closed(endpoint) => {
                let dci;
                if let Direction::In = endpoint.get_direction() {
                    dci = endpoint.endpoint_number() + 2;
                } else {
                    dci = endpoint.endpoint_number() + 1;
                }
                let context_entries =
                    unsafe { *((xhci_device.get_device_context(0) + 3) as *const u8) } >> 3;
                if dci > context_entries {
                    unsafe {
                        memcpy(
                            (&raw mut xhci_device.input_context.slot_context) as *mut c_void,
                            xhci_device.get_device_context(0) as *const c_void,
                            0x20,
                        );
                    };
                    xhci_device.input_context.slot_context.set_part(
                        structures::contexts::XhciSlotContext32Part::ContextEntries,
                        dci as u32,
                    );
                }

                for i in 0..7 {
                    let normal_trb = RawXhciNormalTrb::from_mut(unsafe {
                        endpoint.enqueue_pointer.add(i as usize * 4)
                    });
                    normal_trb.data_buffer = report_address as u64;
                }

                if let Option::Some(callback_fn) = callback {
                    self.pollers.push_back(XhciInterrupPoller {
                        r#fn: callback_fn,
                        dev_addr: xhci_device.device_address(),
                        epid: dci,
                    });
                }

                xhci_device.input_context.input_context_configuration.a_line = 1 << dci | 1;
                self.command_ring
                    .process(structures::XhciCommand::ConfigureEndpoint {
                        slot_id: xhci_device.slot_id as u8,
                        input_context_address: (&raw const *xhci_device.input_context) as u64,
                        dc: false,
                    });
                self.interrupter.wait_for_events_or_crash(
                    self.module.name(),
                    "Timeout while waiting for a Response from ConfigureEndpoint\n",
                );
                xhci_device.input_context.input_context_configuration.a_line = 0;
            }
        }
        /** If the borrow check would be alive, I would be imprisoned for torture. */
        let raw_endpoint_ptr = (&raw const *endpoint) as *mut XhciEndpointDescriptor;
        match &endpoint.endpoint {
            structures::endpoint::XhciEndpointDescriptorRealEndpoint::Closed(addr) => {
                let ptr = (&raw const addr) as *mut *const *mut XhciEndpoint;
                let mut_ep = unsafe { **ptr };
                unsafe {
                    (*raw_endpoint_ptr).endpoint =
                        structures::endpoint::XhciEndpointDescriptorRealEndpoint::Open(
                            &mut *mut_ep,
                        );
                }
            }
            _ => {}
        }
    }
    fn number_of_active_devices(&self) -> u16 {
        return self.interface_data.active_devices as u16;
    }
    fn number_of_potential_devices(&self) -> u16 {
        return self.bar.hcsparams1().get(XhciHcsParams1Part::MaxPorts) as u16;
    }
    fn start(&mut self) {
        if self.bar.usbsts().is_set(XhciUsbStsBitPart::HcH) {
            self.bar.usbcmd().set(XhciUsbCmdBitPart::Rs, true);
            while self.bar.usbsts().is_set(XhciUsbStsBitPart::HcH) {}
        }
    }
    fn stop(&mut self) {
        self.bar.usbcmd().set(XhciUsbCmdBitPart::Rs, false);
        while !self.bar.usbsts().is_set(XhciUsbStsBitPart::HcH) {}
    }
    fn untraited_work0(&mut self) -> Option<bool> {
        let module_name = self.module.name();
        info!(
            &mut self.module,
            "Device Sleep (U1/U2) is not supported & disabled\n"
        );
        for i in 1..self.bar.hcsparams1().get(XhciHcsParams1Part::MaxPorts) as u8 {
            let mut port = self.bar.portsc(i);
            if port.is_set(XhciPortScBitPart::Ccs) {
                port.set(XhciPortScBitPart::Pr, true);
                while port.is_set(XhciPortScBitPart::Pr) {}
                if !self.interrupter.wait_for_events() {
                    simple_kernel_panic(
                        self.module.name(),
                        "Timeout while waiting for Port Change Event\n",
                    );
                }
                let port_speed_as_str = self.get_speed_for_port(i).unwrap().as_str();
                info!(
                    &mut self.module,
                    "Port {} is connected to a {} Device\n", i, port_speed_as_str
                );
                let mut slot_index = 0;

                let mut enable_slot_closure = |trb: *mut RawXhciTrb, trb_type: XhciTrbId| {
                    if let XhciTrbId::CommandCompletionEvent = trb_type {
                        let cc = XhciCommandCompletionEventTrb::from(trb);
                        assert_eq!(
                            cc.get_part(
                                structures::XhciCommandCompletionEventTrbPart::CompletionCode
                            ),
                            1
                        );
                        slot_index =
                            cc.get_part(structures::XhciCommandCompletionEventTrbPart::SlotId);
                    } else {
                        simple_kernel_panic(module_name, "Invalid event\n")
                    }
                };
                {
                    let enable_slot_callback: *mut dyn FnMut(*mut RawXhciTrb, XhciTrbId) =
                        (&mut enable_slot_closure) as *mut dyn FnMut(*mut RawXhciTrb, XhciTrbId);
                    let enable_slot_callback_original_ptr = &raw const enable_slot_callback as u64;
                    let enable_slot_callback_tempered_ptr = enable_slot_callback_original_ptr
                        as *const *mut dyn FnMut(*mut RawXhciTrb, XhciTrbId);
                    let original_callback_fn = unsafe { *enable_slot_callback_tempered_ptr };
                    self.event_callback_fn = Option::Some(original_callback_fn);
                }
                self.command_ring
                    .process(structures::XhciCommand::EnableSlot {
                        slot_type: XHCI_SLOT_TYPE_GENERAL,
                    });
                self.interrupter.wait_for_events_or_crash(
                    module_name,
                    "Timeout while waiting for the Response of EnableSlot\n",
                );
                self.event_callback_fn = Option::None;
                self.dcbaap[slot_index as usize] =
                    self.interface_data.request_device_context(&mut self.module);
                let input_context = self.interface_data.request_input_context(&mut self.module);
                let device = self.interface_data.request_device();
                let mut address_device_closure = |trb: *mut RawXhciTrb, trb_type: XhciTrbId| {
                    if let XhciTrbId::CommandCompletionEvent = trb_type {
                        let cc = XhciCommandCompletionEventTrb::from(trb);
                        assert_eq!(
                            cc.get_part(
                                structures::XhciCommandCompletionEventTrbPart::CompletionCode
                            ),
                            1
                        );
                    } else {
                        simple_kernel_panic(module_name, "Invalid event\n")
                    }
                };
                {
                    let callback: XhciEventCallback =
                        (&mut address_device_closure) as XhciEventCallback;
                    let callback_original_ptr = &raw const callback as u64;
                    let callback_tempered_ptr = callback_original_ptr as *const XhciEventCallback;
                    let original_callback_fn = unsafe { *callback_tempered_ptr };
                    self.event_callback_fn = Option::Some(original_callback_fn);
                }
                input_context.input_context_configuration.a_line = 3;
                input_context.slot_context.initialize_for_address_device(
                    0,
                    port.get(XhciPortScPart::PortSpeed) as u8,
                    i,
                    0,
                    0,
                    0,
                );
                let max_packet_size = self
                    .get_speed_for_port(i as u8)
                    .unwrap()
                    .max_packet_size_for_device_address();
                let tr_dequeue_pointer = alloc_zero_or_crash(
                    unsafe { &mut *XHCI_TEMPORARY_ALLOCATOR },
                    1,
                    &mut self.module,
                    "Could not allocate Transfer Ring for default Control Endpoint\n",
                )
                .as_mut_ptr::<c_void>() as u64;
                input_context
                    .default_control_endpoint
                    .initialize_for_address_device(
                        4,
                        max_packet_size,
                        tr_dequeue_pointer,
                        max_packet_size / 3,
                    );
                self.command_ring
                    .process(structures::XhciCommand::AddressDevice {
                        slot_id: slot_index as u8,
                        input_context_address: (&raw const *input_context).as_u64(),
                        bsr: false,
                    });
                self.interrupter.wait_for_events_or_crash(
                    module_name,
                    "Timeout while waiting for the Response of AddressDevice\n",
                );
                self.event_callback_fn = Option::None;
                let default_control_endpoint = self.interface_data.request_endpoint();
                *default_control_endpoint = XhciEndpoint::new(
                    tr_dequeue_pointer as *mut u32,
                    max_packet_size,
                    0,
                    crate::drivers::usb::independent::Direction::TdDependent,
                    crate::drivers::usb::independent::UsbTransferType::Control,
                    self.bar.doorbell(slot_index as u16),
                );
                *device = XhciDevice::new(
                    slot_index as u16,
                    i,
                    self.dcbaap[slot_index as usize],
                    input_context,
                    default_control_endpoint,
                );
                device.set_address();
                info!(
                    &mut self.module,
                    "Device at Port {} has Address {}\n",
                    i,
                    device.device_address()
                )
            }
        }
        return Option::Some(true);
    }
    fn untraited_work1(&mut self) -> Option<bool> {
        return Option::None;
    }
    fn untraited_work2(&mut self) -> Option<bool> {
        let mut trb_mb = alloc_zero_or_crash(
            unsafe { &mut *XHCI_TEMPORARY_ALLOCATOR },
            1,
            &mut self.module,
            "Could not allocate Trb Array\n",
        );
        let mut trbs_used = 0;

        for i in 0..self.interface_data.active_devices as usize {
            let protocol = self
                .get_protocol_for_port(self.interface_data.devices[i as usize].get_port())
                .unwrap();
            let device = unsafe { &mut *(&raw mut self.interface_data.devices[i as usize]) };
            let slot_device_context_ptr = device.get_device_context(0) as *const c_void;
            let slot_id = device.slot_id;
            for interface in &mut *device.configuration.interfaces {
                for endpoint in &mut *interface.endpoints {
                    let real_endpoint = self.interface_data.request_endpoint();
                    if trbs_used + 8 > 256 {
                        trb_mb = alloc_zero_or_crash(
                            unsafe { &mut *XHCI_TEMPORARY_ALLOCATOR },
                            1,
                            &mut self.module,
                            "Could not allocate Trb Array\n",
                        );
                        trbs_used = 0;
                    }
                    let trbs = unsafe { trb_mb.as_mut_ptr::<RawXhciTrb>().add(trbs_used as usize) };

                    unsafe {
                        let normal = RawXhciNormalTrb::from_mut(trbs);

                        for i in 0..7 {
                            let normal = RawXhciNormalTrb::from_mut(trbs.add(i));
                            normal.data_buffer = 0;
                            normal.set_type();
                            normal.set(structures::XhciNormalTrbBitPart::Bei, false);
                            normal.set(structures::XhciNormalTrbBitPart::Idt, false);
                            normal.set(structures::XhciNormalTrbBitPart::Ioc, true);
                            normal.set(structures::XhciNormalTrbBitPart::Ch, false);
                            normal.set(structures::XhciNormalTrbBitPart::Ns, false);
                            normal.set(structures::XhciNormalTrbBitPart::Isp, false);
                            normal.set(structures::XhciNormalTrbBitPart::Ent, false);
                            normal.set(structures::XhciNormalTrbBitPart::C, true);
                            normal.set_interrupter_target(0);
                            normal.set_td_size(0);
                            normal
                                .set_trb_transfer_length(endpoint.get_maximum_packet_size() as u32);
                        }
                        let mut link = XhciLinkTrb::from(trbs.add(7));
                        link.set_ring_segment_pointer(trbs as u64);
                        link.set_part(structures::XhciLinkTrbPart::InterrupterTarget, 0);
                        link.set_part(structures::XhciLinkTrbPart::TrbType, XhciTrbId::Link as u32);
                        link.set(structures::XhciLinkTrbBitPart::C, true);
                        link.set(structures::XhciLinkTrbBitPart::Ch, false);
                        link.set(structures::XhciLinkTrbBitPart::Ioc, false);
                        link.set(structures::XhciLinkTrbBitPart::Tc, true);
                    }
                    trbs_used += 8;
                    /* TODO: this!*/
                    *real_endpoint = XhciEndpoint::new(
                        trbs as *mut u32,
                        endpoint.get_maximum_packet_size(),
                        endpoint.endpoint_number(),
                        endpoint.get_direction(),
                        endpoint.get_transfer_type(),
                        self.bar.doorbell(slot_id),
                    );
                    let dci;
                    if let Direction::In = endpoint.get_direction() {
                        dci = endpoint.endpoint_number() + 2;
                    } else {
                        dci = endpoint.endpoint_number() + 1;
                    }

                    {
                        let desc = &mut device.input_context.endpoints[dci as usize - 2];
                        desc.set_part(structures::contexts::XhciEndpointContext32Part::EpState, 3);

                        match protocol {
                            UsbProtocol::Usb3 | UsbProtocol::Usb3_1 | UsbProtocol::Usb3_2 => {
                                desc.set_part(
                                    structures::contexts::XhciEndpointContext32Part::MaxBurstSize,
                                    endpoint.superspeed_max_burst as u32,
                                );

                                if let super::independent::UsbTransferType::Isochronous =
                                    endpoint.get_transfer_type()
                                {
                                    desc.set_part(
                                        structures::contexts::XhciEndpointContext32Part::Mult,
                                        endpoint.superspeed_bm_attributes as u32 & 3,
                                    );
                                    desc.set_part(structures::contexts::XhciEndpointContext32Part::MaxPStreams, 0);
                                } else {
                                    if let UsbTransferType::Bulk = endpoint.get_transfer_type() {
                                        desc.set_part(structures::contexts::XhciEndpointContext32Part::MaxPStreams, endpoint.superspeed_bm_attributes as u32 & 0x1F);
                                        if endpoint.superspeed_bm_attributes as u32 & 0x1F != 0 {
                                            todo!("Implement Bulk endpoints with streams!\n")
                                        }
                                    }
                                    desc.set_part(
                                        structures::contexts::XhciEndpointContext32Part::Mult,
                                        0,
                                    );
                                }
                            }
                            UsbProtocol::Usb1 | UsbProtocol::Usb2 => {
                                desc.set_part(
                                    structures::contexts::XhciEndpointContext32Part::Mult,
                                    0,
                                );
                                desc.set_part(
                                    structures::contexts::XhciEndpointContext32Part::MaxBurstSize,
                                    0,
                                );
                                desc.set_part(
                                    structures::contexts::XhciEndpointContext32Part::MaxPStreams,
                                    0,
                                );
                            }
                        }
                        desc.set(
                            structures::contexts::XhciEndpointContext32BitPart::Lsa,
                            false,
                        );
                        desc.set_part(
                            structures::contexts::XhciEndpointContext32Part::Interval,
                            endpoint.interval as u32,
                        );

                        if let UsbTransferType::Bulk = endpoint.get_transfer_type() {
                            desc.set_max_esit_payload(0);
                        } else {
                            desc.set_max_esit_payload(endpoint.get_maximum_packet_size() as u32 * (desc.get_part(structures::contexts::XhciEndpointContext32Part::Mult) + 1) * (desc.get_part(structures::contexts::XhciEndpointContext32Part::MaxBurstSize) + 1));
                        }
                        desc.set_part(structures::contexts::XhciEndpointContext32Part::CErr, 3);
                        desc.set_part(
                            structures::contexts::XhciEndpointContext32Part::EpType,
                            match (endpoint.get_transfer_type(), endpoint.get_direction()) {
                                (UsbTransferType::Isochronous, Direction::Out) => 1,
                                (UsbTransferType::Bulk, Direction::Out) => 2,
                                (UsbTransferType::Interrupt, Direction::Out) => 3,
                                (UsbTransferType::Isochronous, Direction::In) => 5,
                                (UsbTransferType::Bulk, Direction::In) => 6,
                                (UsbTransferType::Interrupt, Direction::In) => 7,
                                (_, _) => {
                                    simple_kernel_panic(self.module.name(), "Type not supported\n")
                                }
                            },
                        );
                        desc.set(
                            structures::contexts::XhciEndpointContext32BitPart::Hid,
                            false,
                        );
                        desc.set_part(
                            structures::contexts::XhciEndpointContext32Part::MaxPacketSize,
                            endpoint.get_maximum_packet_size() as u32,
                        );
                        desc.set(
                            structures::contexts::XhciEndpointContext32BitPart::Dcs,
                            true,
                        );
                        desc.set_tr_dequeue_pointer(trbs as u64);

                        if let UsbTransferType::Bulk = endpoint.get_transfer_type() {
                            desc.set_part(
                                structures::contexts::XhciEndpointContext32Part::AverageTrbLength,
                                endpoint.get_maximum_packet_size() as u32 / 2,
                            );
                        } else {
                            desc.set_part(
                                structures::contexts::XhciEndpointContext32Part::AverageTrbLength,
                                endpoint.get_maximum_packet_size() as u32,
                            );
                        }
                    }
                    endpoint.endpoint =
                        structures::endpoint::XhciEndpointDescriptorRealEndpoint::Closed(
                            real_endpoint,
                        );
                }
            }
        }

        return Option::None;
    }
}
pub fn create_xhci(
    pci_bus: &PciBus,
    pci_device: u64,
    physical_allocator: &mut Allocator,
    isr_vector: u8,
) {
    unsafe {
        let xhci_controller = &raw mut XHCI_CONTROLLER;
        XHCI_TEMPORARY_ALLOCATOR = &raw mut *physical_allocator;
        (*xhci_controller).initialize_controller(
            pci_bus,
            physical_allocator,
            isr_vector,
            pci_device,
        );
        (*xhci_controller).untraited_work0();
        (*xhci_controller).gather_device_information();
        (*xhci_controller).configure_devices();
        (*xhci_controller).untraited_work2();
    }
}
