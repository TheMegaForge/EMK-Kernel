use core::slice;

pub struct SataCommandHeader {
    data: &'static mut [u32],
}

impl SataCommandHeader {
    pub fn new(data: *mut u32) -> Self {
        return Self {
            data: unsafe { slice::from_raw_parts_mut(data.as_mut().unwrap(), 4) },
        };
    }

    pub fn zero_out(&mut self) {
        self.data[0] = 0;
        self.data[1] = 0;
        self.data[2] = 0;
        self.data[3] = 0;
    }

    pub fn set_command_fis_length(&mut self, mut val: u8) {
        val &= 0b11111;
        self.data[0] = (self.data[0] & !0b11111) | (val as u32);
    }
    pub fn set_atapi(&mut self, val: bool) {
        self.data[0] = (self.data[0] & !(1 << 5)) | (val as u32) << 5;
    }
    pub fn set_write(&mut self, val: bool) {
        self.data[0] = (self.data[0] & !(1 << 6)) | (val as u32) << 6;
    }
    pub fn set_prefetchable(&mut self, val: bool) {
        self.data[0] = (self.data[0] & !(1 << 7)) | (val as u32) << 7;
    }
    pub fn set_reset(&mut self, val: bool) {
        self.data[0] = (self.data[0] & !(1 << 8)) | (val as u32) << 8;
    }
    pub fn set_bist(&mut self, val: bool) {
        self.data[0] = (self.data[0] & !(1 << 9)) | (val as u32) << 9;
    }
    pub fn clear_busy_upon_r_ok(&mut self, val: bool) {
        self.data[0] = (self.data[0] & !(1 << 10)) | (val as u32) << 10;
    }
    pub fn set_port_multiplier_port(&mut self, mut val: u8) {
        val &= 0b1111;
        self.data[0] = (self.data[0] & !(0b1111 << 12)) | (val as u32) << 12;
    }
    pub fn set_physical_region_descriptor_table_length(&mut self, val: u16) {
        self.data[0] = self.data[0] & !(0xFFFF << 16) | (val as u32) << 16;
    }
    pub fn physical_region_descriptor_byte_count(&self) -> u32 {
        return self.data[1];
    }
    pub fn set_command_table_descriptor_base_address(&mut self, base: u32) {
        self.data[2] = (base >> 7) << 7;
    }
    pub fn set_command_table_base_address_upper(&mut self, addr: u32) {
        self.data[3] = addr;
    }
}

pub struct SataPRDTEntry {
    data: &'static mut [u32],
}

impl SataPRDTEntry {
    pub fn new(data: *mut u32) -> Self {
        return Self {
            data: unsafe { slice::from_raw_parts_mut(data.as_mut().unwrap(), 4) },
        };
    }

    pub fn zero_out(&mut self) {
        self.data[0] = 0;
        self.data[1] = 0;
        self.data[2] = 0;
        self.data[3] = 0;
    }

    pub fn set_data_base_address(&mut self, base: u32) {
        self.data[0] = (base >> 1) << 1;
    }
    pub fn set_data_base_address_upper(&mut self, addr: u32) {
        self.data[1] = addr;
    }
    pub fn set_data_byte_count(&mut self, mut count: u32) {
        /* 0x3FFFFF -> Mask */
        count &= 0x3FFFFF;
        self.data[3] = (self.data[3] & !0x3FFFFF) | count;
    }
    pub fn set_interrupt_on_completion(&mut self, val: bool) {
        self.data[3] = (self.data[3] & !(1 << 31)) | (val as u32) << 31;
    }
}
