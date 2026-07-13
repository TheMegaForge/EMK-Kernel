#[repr(C, packed)]
pub struct XhciInputContextConfigurationPart32 {
    pub d_line: u32,
    pub a_line: u32,
    _reserved0: u32,
    _reserved1: u32,
    _reserved2: u32,
    _reserved3: u32,
    _reserved4: u32,
    pub configuration_value: u8,
    pub interface_number: u8,
    pub alternate_setting: u8,
    _reserved5: u8,
}
#[repr(u32)]
pub enum XhciSlotContext32Part {
    Speed = (0xF << 10) | (20 << 4) | 0,
    ContextEntries = (0x1F << 10) | (27 << 4) | 0,
    RootHubPortNumber = (0xFF << 10) | (16 << 4) | 1,
    MaxExitLatency = (0xFFFF << 10) | (0 << 4) | 1,
    NumberOfPorts = (0xFF << 10) | (24 << 4) | 1,
    /** Parent Hub Slot Id*/
    TTHubSlotId = (0xFF << 10) | (0 << 4) | 2,
    /** Parent Port Number*/
    TTPortNumber = (0xFF << 10) | (8 << 4) | 2,
    /** TT Think Time*/
    TTT = (0x3 << 10) | (16 << 4) | 2,
    InterrupterTarget = (0x7FF << 10) | (22 << 4) | 2,
    UsbDeviceAddress = (0xFF << 10) | (0 << 4) | 3,
    SlotState = (0x1F << 10) | (27 << 4) | 3,
}
#[repr(u32)]
pub enum XhciSlotContext32BitPart {
    /** Multi-TT*/
    Mtt = (25 << 16) | 0,
    Hub = (26 << 16) | 0,
}
#[repr(C, packed)]
pub struct XhciSlotContext32 {
    dwords: [u32; 8],
}

impl XhciSlotContext32 {
    pub fn initialize_for_address_device(
        &mut self,
        route_string: u32,
        port_speed: u8,
        root_hub_port: u8,
        tt_hub_slot_id: u8,
        tt_port_number: u8,
        interrupter_target: u16,
    ) {
        self.set_route_string(route_string);
        self.set_part(XhciSlotContext32Part::Speed, port_speed as u32);
        self.set_part(XhciSlotContext32Part::ContextEntries, 1);
        self.set_part(XhciSlotContext32Part::MaxExitLatency, 0);
        self.set_part(
            XhciSlotContext32Part::RootHubPortNumber,
            root_hub_port as u32,
        );
        self.set_part(XhciSlotContext32Part::NumberOfPorts, 0);
        self.set_part(XhciSlotContext32Part::TTHubSlotId, tt_hub_slot_id as u32);
        self.set_part(XhciSlotContext32Part::TTPortNumber, tt_port_number as u32);
        self.set_part(XhciSlotContext32Part::TTT, 0);
        self.set_part(
            XhciSlotContext32Part::InterrupterTarget,
            interrupter_target as u32,
        );
        self.set_part(XhciSlotContext32Part::UsbDeviceAddress, 0);
        self.set_part(XhciSlotContext32Part::SlotState, 0);
    }

    pub fn is_set(&self, bit_part: XhciSlotContext32BitPart) -> bool {
        let part_u32 = bit_part as u32;
        let val = self.dwords[(part_u32 & 0xF) as usize];
        return 1 == val >> (part_u32 >> 16) & 1;
    }

    pub fn set(&mut self, bit_part: XhciSlotContext32BitPart, val: bool) {
        let part_u32 = bit_part as u32;
        let mut prev_val = self.dwords[(part_u32 & 0xF) as usize];
        prev_val &= !(1 << (part_u32 >> 16));
        prev_val |= (val as u32) << (part_u32 >> 16);
        self.dwords[(part_u32 & 0xF) as usize] = prev_val;
    }

    pub fn get_part(&self, part: XhciSlotContext32Part) -> u32 {
        let part_u32 = part as u32;
        let val = self.dwords[(part_u32 & 0xF) as usize];
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: XhciSlotContext32Part, val: u32) {
        let part_u32 = part as u32;
        let mut prev_val = self.dwords[(part_u32 & 0xF) as usize];
        prev_val &= !((part_u32 >> 10) << ((part_u32 >> 4) & 0x1F));
        prev_val |= (val & (part_u32 >> 10)) << ((part_u32 >> 4) & 0x1F);
        self.dwords[(part_u32 & 0xF) as usize] = prev_val;
    }
    #[inline(always)]
    pub fn set_route_string(&mut self, route_string: u32) {
        let prev_val = self.dwords[0] & !0xFFFFF;
        self.dwords[0] = prev_val | route_string & 0xFFFFF;
    }
    #[inline(always)]
    pub fn get_route_string(&mut self) -> u32 {
        self.dwords[0] & 0xFFFFF
    }
}

#[repr(u32)]
pub enum XhciEndpointContext32Part {
    /** Endpoint State*/
    EpState = (0x7 << 10) | (0 << 4) | 0,
    Mult = (0x3 << 10) | (8 << 4) | 0,
    /** Max Primary Streams*/
    MaxPStreams = (0x1F << 10) | (10 << 4) | 0,
    Interval = (0xFF << 10) | (16 << 4) | 0,
    /** Error Count*/
    CErr = (0x3 << 10) | (1 << 4) | 1,
    /** Endpoint Type*/
    EpType = (0x7 << 10) | (3 << 4) | 1,
    MaxBurstSize = (0xFF << 10) | (8 << 4) | 1,
    MaxPacketSize = (0xFFFF << 10) | (16 << 4) | 1,
    AverageTrbLength = (0xFFFF << 10) | (0 << 4) | 4,
}
#[repr(u32)]
pub enum XhciEndpointContext32BitPart {
    /** Linear Stream Array*/
    Lsa = (15 << 16) | 0,
    /** Host Initiate Disable*/
    Hid = (7 << 16) | 1,
    /** Dequeue Cycle State*/
    Dcs = (0 << 16) | 2,
}
#[repr(C, packed)]
pub struct XhciEndpointContext32 {
    dwords: [u32; 8],
}

impl XhciEndpointContext32 {
    pub fn initialize_for_address_device(
        &mut self,
        ep_type: u8,
        max_packet_size: u16,
        tr_dequeue_pointer: u64,
        average_trb_length: u16,
    ) {
        self.set_part(XhciEndpointContext32Part::EpState, 0);
        self.set_part(XhciEndpointContext32Part::Mult, 0);
        self.set_part(XhciEndpointContext32Part::MaxPStreams, 0);
        self.set(XhciEndpointContext32BitPart::Lsa, false);
        self.set_part(XhciEndpointContext32Part::Interval, 0);
        self.set_max_esit_payload(0);
        self.set_part(XhciEndpointContext32Part::CErr, 3);
        self.set_part(XhciEndpointContext32Part::EpType, ep_type as u32);
        self.set(XhciEndpointContext32BitPart::Hid, true);
        self.set_part(XhciEndpointContext32Part::MaxBurstSize, 0);
        self.set_part(
            XhciEndpointContext32Part::MaxPacketSize,
            max_packet_size as u32,
        );
        self.set(XhciEndpointContext32BitPart::Dcs, true);
        self.set_tr_dequeue_pointer(tr_dequeue_pointer);
        self.set_part(
            XhciEndpointContext32Part::AverageTrbLength,
            average_trb_length as u32,
        );
    }

    pub fn is_set(&self, bit_part: XhciEndpointContext32BitPart) -> bool {
        let part_u32 = bit_part as u32;
        let val = self.dwords[(part_u32 & 0xF) as usize];
        return 1 == val >> (part_u32 >> 16) & 1;
    }

    pub fn set(&mut self, bit_part: XhciEndpointContext32BitPart, val: bool) {
        let part_u32 = bit_part as u32;
        let mut prev_val = self.dwords[(part_u32 & 0xF) as usize];
        prev_val &= !(1 << (part_u32 >> 16));
        prev_val |= (val as u32) << (part_u32 >> 16);
        self.dwords[(part_u32 & 0xF) as usize] = prev_val;
    }

    pub fn get_part(&self, part: XhciEndpointContext32Part) -> u32 {
        let part_u32 = part as u32;
        let val = self.dwords[(part_u32 & 0xF) as usize];
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: XhciEndpointContext32Part, val: u32) {
        let part_u32 = part as u32;
        let mut prev_val = self.dwords[(part_u32 & 0xF) as usize];
        prev_val &= !((part_u32 >> 10) << ((part_u32 >> 4) & 0x1F));
        prev_val |= (val & (part_u32 >> 10)) << ((part_u32 >> 4) & 0x1F);
        self.dwords[(part_u32 & 0xF) as usize] = prev_val;
    }
    /** Max Endpoint Service Time Payload*/
    pub fn set_max_esit_payload(&mut self, payload: u32) {
        let hi = payload >> 16;
        let lo = payload & 0xFFFF;
        let prev_val0 = self.dwords[0] & !(0xFF << 24);
        let prev_val4 = self.dwords[4] & !(0xFFFF << 16);
        self.dwords[0] = prev_val0 | hi << 24;
        self.dwords[4] = prev_val4 | lo << 16;
    }
    /** Max Endpoint Service Time Payload*/
    pub fn get_esit_payload(&self) -> u32 {
        let esit_payload_hi = self.dwords[0] >> 24;
        let esit_payload_lo = self.dwords[4] >> 16;
        return esit_payload_lo | esit_payload_hi << 16;
    }
    #[inline(always)]
    pub fn set_tr_dequeue_pointer(&mut self, pointer: u64) {
        let prev_val = self.dwords[2] & 1;
        self.dwords[2] = prev_val | (pointer & 0xFFFFFFFF) as u32;
        self.dwords[3] = (pointer >> 32) as u32;
    }
    #[inline(always)]
    pub fn get_tr_dequeue_pointer(&self) -> u64 {
        let lo = self.dwords[2] & !1;
        return lo as u64 | (self.dwords[3] as u64) << 32;
    }
}

#[repr(C, packed)]
pub struct XhciInputContext32 {
    pub input_context_configuration: XhciInputContextConfigurationPart32,
    pub slot_context: XhciSlotContext32,
    pub default_control_endpoint: XhciEndpointContext32,
    pub endpoints: [XhciEndpointContext32; 30],
}
