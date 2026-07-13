use core::{ffi::c_void, ptr::null_mut};

use crate::{
    drivers::usb::{
        independent::{Direction, PidCode, UsbTransferType},
        ohci::structures::{
            OHCI_TRANSFER_DESCRIPTOR_PROCESSED,
            device::OhciDevice,
            endpoint::{EndpointDescriptorBitPart::S, EndpointDescriptorPart::Mps},
            transfer_descriptors::{self, GeneralTD, GeneralTDBitPart, GeneralTDPart},
        },
        standard_requests::UsbEndpointDescriptor,
        traits::UsbEndpoint,
    },
    hal::print::simple_kernel_panic,
};
#[derive(Clone, Copy)]
pub struct OhciEndpointDescriptor {
    val: *mut u32,
}
#[repr(C)]
pub struct RawOhciEndpointDescriptor {
    _dword0: u32,
    _dword1: u32,
    _dword2: u32,
    _dword3: u32,
}

/*
 * x << 16, x is the mask
 * x << 4 , x is the starting position
 * | x, is the dword
 */

#[repr(u32)]
pub enum EndpointDescriptorPart {
    /** FunctionAddress*/
    Fa = (0x7F << 10) | (0 << 4) | 0,
    /** Endpoint Number*/
    En = (0xF << 10) | (7 << 4) | 0,
    /** Direction*/
    D = (0x3 << 10) | (11 << 4) | 0,
    /** MaximumPacketSize*/
    Mps = (0x7FF << 10) | (16 << 4) | 0,
}
#[repr(u32)]
pub enum EndpointDescriptorBitPart {
    /** Speed*/
    S = (13 << 16) | 0,
    /** Skip*/
    K = (14 << 16) | 0,
    /** Format*/
    F = (15 << 16) | 0,

    /** Connector (Custom)*/
    Con = (27 << 16) | 0,

    /** Dummy (Custom). Used in the Control List and indicating that this is for a possible device
     *  Also used in the Interrupt List, indicating a free 32ms ed
     */
    Dum = (28 << 16) | 0,

    /** Halted*/
    H = (0 << 16) | 2,
    /** toggleCarry */
    C = (1 << 16) | 2,
}

impl OhciEndpointDescriptor {
    pub fn new(addr: *mut c_void) -> Self {
        return Self {
            val: addr as *mut u32,
        };
    }

    pub fn from_general(
        &mut self,
        function_address: u32,
        speed: bool,
        endpoint: &OhciGeneralEndpoint,
    ) {
        self.set_part(EndpointDescriptorPart::Fa, function_address);
        self.set_part(
            EndpointDescriptorPart::En,
            endpoint.get_endpoint_address() as u32 & 0x7,
        );
        self.set_part(
            EndpointDescriptorPart::D,
            match (endpoint.get_endpoint_address() >> 7) {
                0 => Direction::Out.as_ohci(),
                1 => Direction::In.as_ohci(),
                _ => simple_kernel_panic("OhciEndpointDescriptor/from_general", "Just How?\n"),
            },
        );
        self.set(EndpointDescriptorBitPart::S, speed);
        self.set(EndpointDescriptorBitPart::K, true);
        self.set_part(
            EndpointDescriptorPart::Mps,
            endpoint.get_max_packet_size() as u32,
        );
    }

    pub fn copy_from(&mut self, ep: &OhciEndpointDescriptor) {
        unsafe {
            self.val
                .add(0)
                .write_volatile(ep.val.add(0).read_volatile());
            self.val
                .add(1)
                .write_volatile(ep.val.add(1).read_volatile());
            self.val
                .add(2)
                .write_volatile(ep.val.add(2).read_volatile());
            self.val
                .add(3)
                .write_volatile(ep.val.add(3).read_volatile());
        }
    }

    pub fn address(&self) -> u32 {
        return self.val as u32;
    }

    pub fn zero_out(&mut self) {
        unsafe {
            self.val.add(0).write_volatile(0);
            self.val.add(1).write_volatile(0);
            self.val.add(2).write_volatile(0);
            self.val.add(3).write_volatile(0);
        }
    }

    pub fn is_set(&self, bit_part: EndpointDescriptorBitPart) -> bool {
        let part_u32 = bit_part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return 1 == val >> (part_u32 >> 16) & 1;
    }

    pub fn set(&mut self, bit_part: EndpointDescriptorBitPart, val: bool) {
        let part_u32 = bit_part as u32;
        let mut prev_val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        prev_val &= !(1 << (part_u32 >> 16));
        prev_val |= (val as u32) << (part_u32 >> 16);
        unsafe {
            self.val
                .add((part_u32 & 0xF) as usize)
                .write_volatile(prev_val)
        }
    }

    pub fn get_part(&self, part: EndpointDescriptorPart) -> u32 {
        let part_u32 = part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: EndpointDescriptorPart, val: u32) {
        let part_u32 = part as u32;
        let mut prev_val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        prev_val &= !((part_u32 >> 10) << ((part_u32 >> 4) & 0x1F));
        prev_val |= (val & (part_u32 >> 10)) << ((part_u32 >> 4) & 0x1F);
        unsafe {
            self.val
                .add((part_u32 & 0xF) as usize)
                .write_volatile(prev_val)
        }
    }

    pub fn tail_p(&self) -> u32 {
        unsafe { self.val.add(1).read_volatile() }
    }
    pub fn head_p(&self) -> u32 {
        (unsafe { self.val.add(2).read_volatile() } >> 4) << 4
    }
    pub fn next_ed(&self) -> u32 {
        unsafe { self.val.add(3).read_volatile() }
    }

    pub fn write_tail_p(&mut self, val: u32) {
        unsafe { self.val.add(1).write_volatile(val) }
    }
    pub fn write_head_p(&mut self, val: u32) {
        let mut prev_val = unsafe { self.val.add(2).read_volatile() };
        unsafe { self.val.add(2).write_volatile(val | prev_val & 0x3) }
    }
    pub fn write_next_ed(&mut self, val: u32) {
        unsafe { self.val.add(3).write_volatile(val) }
    }

    /**
     * Get´s the last ep in the ep list
     * Returns Option::None, if self.next_ed() == 0
     */
    pub fn get_last_ep(&self) -> Option<OhciEndpointDescriptor> {
        if self.next_ed() == 0 {
            return Option::None;
        }
        let mut ret = OhciEndpointDescriptor::new(self.next_ed() as *mut c_void);
        while ret.next_ed() != 0 {
            ret = OhciEndpointDescriptor::new(ret.next_ed() as *mut c_void);
        }
        return Option::Some(ret);
    }
}
/* Isochronous/Interrupt */
pub struct OhciPeriodicEndpoint {
    descriptor: OhciEndpointDescriptor,
    transfer_type: UsbTransferType,
    pub(in crate::drivers::usb::ohci) transfer_descriptors: *mut c_void,
    maximum_transfer_descriptors: u8,
    interval: u8,
}

impl OhciPeriodicEndpoint {
    pub fn new(descriptor: OhciEndpointDescriptor, transfer_type: UsbTransferType) -> Self {
        return Self {
            descriptor,
            transfer_type,
            transfer_descriptors: null_mut(),
            maximum_transfer_descriptors: 0,
            interval: 0,
        };
    }
    pub fn set_interval(&mut self, interval: u8) {
        self.interval = interval;
    }

    pub fn get_endpoint_descriptor(&self) -> OhciEndpointDescriptor {
        return self.descriptor;
    }
}

impl UsbEndpoint for OhciPeriodicEndpoint {
    fn get_interval_in_ms(&self) -> u16 {
        return self.interval as u16;
    }
    fn endpoint_number(&self) -> u8 {
        return self.descriptor.get_part(EndpointDescriptorPart::En) as u8;
    }
    fn get_direction(&self) -> Direction {
        return Direction::from_ohci(self.descriptor.get_part(EndpointDescriptorPart::D));
    }
    fn get_maximum_packet_size(&self) -> u16 {
        return self.descriptor.get_part(EndpointDescriptorPart::Mps) as u16;
    }
    fn get_transfer_type(&self) -> UsbTransferType {
        return self.transfer_type;
    }
    fn set_address_and_length(&mut self, base: *mut c_void, maximum_qtds: u8) {
        self.maximum_transfer_descriptors = maximum_qtds;
        self.transfer_descriptors = base
    }
    fn update_max_packet_size(&mut self, max_packet_size: u16) {
        self.descriptor.set_part(Mps, max_packet_size as u32);
    }
}

/* Control/ Bulk*/
pub struct OhciNonPeriodicEndpoint {
    descriptor: OhciEndpointDescriptor,
    transfer_descriptors: *mut c_void,
    maximum_transfer_descriptors: u8,
    transfer_type: UsbTransferType,
}

impl OhciNonPeriodicEndpoint {
    pub fn new(transfer_type: UsbTransferType, descriptor: OhciEndpointDescriptor) -> Self {
        return Self {
            descriptor,
            transfer_descriptors: null_mut(),
            maximum_transfer_descriptors: 0,
            transfer_type,
        };
    }

    pub fn get_endpoint_descriptor(&self) -> OhciEndpointDescriptor {
        return self.descriptor;
    }

    /**
     * Notice: If a zero length transfer should be executed, setup_buffer_length or status_buffer_length has to be set to 1 and setup_buffer or status_buffer has to be 0
     * Notice: exact_fit is for the status and the setup transfer descriptor
     * Info: for more Information about exact_fit read Section 4.3.1.2 of the OHCI Specification
     *
     */
    pub fn send_setup_status(
        &mut self,
        interrupt_delay: u8,
        setup_buffer: u32,
        setup_buffer_length: u32,
        status_buffer: u32,
        status_buffer_length: u32,
        exact_fit: bool,
    ) {
        let mut setup_td = GeneralTD::new(self.transfer_descriptors as *mut u32);
        let mut status_td =
            GeneralTD::new(unsafe { self.transfer_descriptors.add(16) } as *mut u32);
        setup_td.zero_out();
        status_td.zero_out();

        status_td.set_part(GeneralTDPart::Dp, PidCode::InToken.as_ohci());
        status_td.set_part(GeneralTDPart::Di, interrupt_delay as u32);

        setup_td.set_part(GeneralTDPart::Dp, PidCode::SetupToken.as_ohci());
        setup_td.set_part(GeneralTDPart::Di, 0b111); // setup td generates no interrupt

        setup_td.set(GeneralTDBitPart::R, !exact_fit);
        setup_td.write_cbp(setup_buffer);
        setup_td.write_buffer_end(setup_buffer + (setup_buffer_length - 1));

        status_td.write_cbp(status_buffer);
        status_td.write_buffer_end(status_buffer + (status_buffer_length - 1));

        let endpoint_number = self.descriptor.get_part(EndpointDescriptorPart::En);

        /* This is needed, so that the OHCI will process the Status Td*/
        /* 20 << 2 notates the number of total transfer descriptor, being 2*/
        if let UsbTransferType::Control = self.transfer_type {
            status_td.write_next_td(
                OHCI_TRANSFER_DESCRIPTOR_PROCESSED
                    | 2 << 20
                    | self.descriptor.get_part(EndpointDescriptorPart::En) << 10,
            );
        } else {
            status_td.write_next_td(OHCI_TRANSFER_DESCRIPTOR_PROCESSED | 2 << 20);
        }
        setup_td.write_next_td(status_td.address());

        setup_td.set_part(GeneralTDPart::T, 0b10); // Data0
        status_td.set_part(GeneralTDPart::T, 0b11); // Data1

        self.descriptor
            .write_head_p(self.transfer_descriptors as u32);
        self.descriptor.write_tail_p(status_td.next_td());
        self.descriptor.set(EndpointDescriptorBitPart::K, false);
    }

    pub fn send_setup_out_status(
        &mut self,
        interrupt_delay: u8,
        setup_buffer: u32,
        setup_buffer_length: u32,
        out_buffer: u32,
        out_buffer_length: u32,
        exact_fit: bool,
    ) {
        let mut setup_td = GeneralTD::new(self.transfer_descriptors as *mut u32);
        let mut out_td = GeneralTD::new(unsafe { self.transfer_descriptors.add(16) } as *mut u32);
        let mut status_td =
            GeneralTD::new(unsafe { self.transfer_descriptors.add(32) } as *mut u32);
        setup_td.zero_out();
        status_td.zero_out();
        out_td.zero_out();

        status_td.set_part(GeneralTDPart::Dp, PidCode::InToken.as_ohci());
        status_td.set_part(GeneralTDPart::Di, interrupt_delay as u32);

        out_td.set_part(GeneralTDPart::Dp, PidCode::OutToken.as_ohci());
        out_td.set_part(GeneralTDPart::Di, 0b111);

        setup_td.set_part(GeneralTDPart::Dp, PidCode::SetupToken.as_ohci());
        setup_td.set_part(GeneralTDPart::Di, 0b111); // setup td generates no interrupt

        setup_td.set(GeneralTDBitPart::R, !exact_fit);
        setup_td.write_cbp(setup_buffer);
        setup_td.write_buffer_end(setup_buffer + (setup_buffer_length - 1));

        out_td.set(GeneralTDBitPart::R, !exact_fit);
        out_td.write_cbp(out_buffer);
        out_td.write_buffer_end(out_buffer + (out_buffer_length - 1));

        let endpoint_number = self.descriptor.get_part(EndpointDescriptorPart::En);

        /* This is needed, so that the OHCI will process the Status Td*/
        /* 20 << 2 notates the number of total transfer descriptor, being 2*/
        if let UsbTransferType::Control = self.transfer_type {
            status_td.write_next_td(
                OHCI_TRANSFER_DESCRIPTOR_PROCESSED
                    | 2 << 20
                    | self.descriptor.get_part(EndpointDescriptorPart::En) << 10,
            );
        } else {
            status_td.write_next_td(OHCI_TRANSFER_DESCRIPTOR_PROCESSED | 2 << 20);
        }
        setup_td.write_next_td(status_td.address());
        out_td.write_next_td(status_td.address());

        setup_td.set_part(GeneralTDPart::T, 0b10); // Data0
        out_td.set_part(GeneralTDPart::T, 0b11); // Data1
        status_td.set_part(GeneralTDPart::T, 0b11); // Data1

        self.descriptor
            .write_head_p(self.transfer_descriptors as u32);
        self.descriptor.write_tail_p(status_td.next_td());
        self.descriptor.set(EndpointDescriptorBitPart::K, false);
    }
}

impl UsbEndpoint for OhciNonPeriodicEndpoint {
    fn get_interval_in_ms(&self) -> u16 {
        return 0xFFFF;
    }

    fn endpoint_number(&self) -> u8 {
        return self.descriptor.get_part(EndpointDescriptorPart::En) as u8;
    }
    fn get_direction(&self) -> Direction {
        return Direction::from_ohci(self.descriptor.get_part(EndpointDescriptorPart::D));
    }
    fn get_maximum_packet_size(&self) -> u16 {
        return self.descriptor.get_part(EndpointDescriptorPart::Mps) as u16;
    }
    fn get_transfer_type(&self) -> UsbTransferType {
        return self.transfer_type;
    }
    fn set_address_and_length(&mut self, base: *mut c_void, maximum_qtds: u8) {
        self.maximum_transfer_descriptors = maximum_qtds;
        self.transfer_descriptors = base
    }
    fn update_max_packet_size(&mut self, max_packet_size: u16) {
        self.descriptor.set_part(Mps, max_packet_size as u32);
    }
}

pub enum OhciGeneralEndpointRealEndpoint {
    Unassigned,
    NonPeriodic(&'static mut OhciNonPeriodicEndpoint),
    Periodic(&'static mut OhciPeriodicEndpoint),
}

pub struct OhciGeneralEndpoint {
    endpoint_address: u8,
    bm_attributes: u8,
    w_max_packet_size: u16,
    pub(in crate::drivers::usb::ohci) b_interval: u8,
    pub(in crate::drivers::usb::ohci) real_endpoint: OhciGeneralEndpointRealEndpoint,
}

impl OhciGeneralEndpoint {
    pub fn from_raw(r#in: &UsbEndpointDescriptor) -> Self {
        return Self {
            endpoint_address: r#in.b_endpoint_address,
            bm_attributes: r#in.bm_attributes,
            w_max_packet_size: r#in.w_max_packet_size,
            b_interval: r#in.b_interval,
            real_endpoint: OhciGeneralEndpointRealEndpoint::Unassigned,
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
