pub struct GeneralTD {
    val: *mut u32,
}

#[repr(u32)]
pub enum GeneralTDPart {
    Dp = (0x3 << 10) | (19 << 4) | 0,
    Di = (0x7 << 10) | (21 << 4) | 0,
    T = (0x3 << 10) | (24 << 4) | 0,
    Ec = (0x3 << 10) | (26 << 4) | 0,
    Cc = (0xF << 10) | (28 << 4) | 0,
}
#[repr(u32)]
pub enum GeneralTDBitPart {
    R = (18 << 16) | 0,
}

impl GeneralTD {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn zero_out(&mut self) {
        unsafe {
            self.val.add(0).write_volatile(0);
            self.val.add(1).write_volatile(0);
            self.val.add(2).write_volatile(0);
            self.val.add(3).write_volatile(0);
        }
    }

    pub fn is_set(&self, bit_part: GeneralTDBitPart) -> bool {
        let part_u32 = bit_part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return 1 == val >> (part_u32 >> 16) & 1;
    }

    pub fn set(&mut self, bit_part: GeneralTDBitPart, val: bool) {
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

    pub fn get_part(&self, part: GeneralTDPart) -> u32 {
        let part_u32 = part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: GeneralTDPart, val: u32) {
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

    pub fn cbp(&self) -> u32 {
        unsafe { self.val.add(1).read_volatile() }
    }
    pub fn next_td(&self) -> u32 {
        unsafe { self.val.add(2).read_volatile() }
    }
    pub fn buffer_end(&self) -> u32 {
        unsafe { self.val.add(3).read_volatile() }
    }

    pub fn write_cbp(&mut self, val: u32) {
        unsafe { self.val.add(1).write_volatile(val) }
    }
    pub fn write_next_td(&mut self, val: u32) {
        unsafe { self.val.add(2).write_volatile((val >> 4) << 4) }
    }
    pub fn write_buffer_end(&mut self, val: u32) {
        unsafe { self.val.add(3).write_volatile((val >> 4) << 4) }
    }
}

#[repr(u32)]
pub enum IsochTDPart {
    Sf = (0xFFFF << 10) | (0 << 4) | 0,
    Di = (0x7 << 10) | (21 << 4) | 0,
    Fc = (0x7 << 10) | (24 << 4) | 0,
    Cc = (0xF << 10) | (28 << 4) | 0,
}

pub struct IsochTD {
    val: *mut u32,
}

impl IsochTD {
    pub fn new(val: *mut u32) -> Self {
        return Self { val };
    }

    pub fn zero_out(&mut self) {
        unsafe {
            self.val.add(0).write_volatile(0);
            self.val.add(1).write_volatile(0);
            self.val.add(2).write_volatile(0);
            self.val.add(3).write_volatile(0);
            self.val.add(4).write_volatile(0);
            self.val.add(5).write_volatile(0);
            self.val.add(6).write_volatile(0);
            self.val.add(7).write_volatile(0);
        }
    }

    pub fn get_part(&self, part: IsochTDPart) -> u32 {
        let part_u32 = part as u32;
        let val = unsafe { self.val.add((part_u32 & 0xF) as usize).read_volatile() };
        return (val >> ((part_u32 >> 4) & 0x1F)) & (part_u32 >> 10);
    }
    pub fn set_part(&mut self, part: IsochTDPart, val: u32) {
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

    pub fn bp0(&self) -> u32 {
        (unsafe { self.val.add(1).read_volatile() } >> 12) << 12
    }
    pub fn next_td(&self) -> u32 {
        (unsafe { self.val.add(2).read_volatile() } >> 4) << 4
    }
    pub fn be(&self) -> u32 {
        unsafe { self.val.add(3).read_volatile() }
    }

    pub fn write_bp0(&mut self, val: u32) {
        let prev_val = unsafe { self.val.add(1).read_volatile() };
        unsafe {
            self.val
                .add(1)
                .write_volatile((val & !0xFFF) | prev_val & 0xFFF)
        }
    }
    pub fn write_next_td(&mut self, val: u32) {
        unsafe { self.val.add(2).write_volatile(val & !0xF) }
    }
    pub fn write_be(&mut self, val: u32) {
        unsafe { self.val.add(3).write_volatile(val) }
    }

    pub fn psw(&mut self, psw: u8) -> u16 {
        assert!(7 >= psw);
        unsafe {
            (self.val.add(4) as *const u16)
                .add(psw as usize)
                .read_volatile()
        }
    }

    pub fn offset(&mut self, offset: u8) -> u16 {
        assert!(7 >= offset);
        unsafe {
            (self.val.add(4) as *const u16)
                .add(offset as usize)
                .read_volatile()
        }
    }

    pub fn write_psw(&mut self, psw: u8, val: u16) {
        assert!(7 >= psw);
        unsafe {
            (self.val.add(4) as *mut u16)
                .add(psw as usize)
                .write_volatile(val);
        }
    }

    pub fn write_offset(&mut self, offset: u8, val: u16) {
        assert!(7 >= offset);
        unsafe {
            (self.val.add(4) as *mut u16)
                .add(offset as usize)
                .write_volatile(val)
        }
    }
}
