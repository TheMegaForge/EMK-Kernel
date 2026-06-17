use core::ffi::c_void;

pub struct NVMeCap<'a> {
    data: &'a u64,
}

impl<'a> NVMeCap<'a> {
    pub fn new(data: &'a u64) -> Self {
        return Self { data };
    }

    pub fn mqes(&self) -> u16 {
        return (self.data & 0xFFFF) as u16;
    }

    pub fn dstrd(&self) -> u8 {
        return ((self.data >> 32) & 0b1111) as u8;
    }

    pub fn css(&self) -> u8 {
        return ((self.data >> 37) & 0b1111_1111) as u8;
    }
}

pub struct NVMeVS<'a> {
    data: &'a u32,
}

impl<'a> NVMeVS<'a> {
    pub fn new(data: &'a u32) -> Self {
        return Self { data };
    }
}

pub struct NVMeCC<'a> {
    data: &'a mut u32,
}

impl<'a> NVMeCC<'a> {
    pub fn new(data: &'a mut u32) -> Self {
        return Self { data };
    }

    pub fn en(&self) -> bool {
        return 1 == *self.data & 1;
    }
    pub fn set_en(&mut self, val: bool) {
        *self.data = (*self.data & !1) | (val as u32);
    }

    pub fn set_css(&mut self, mut val: u8) {
        val &= 0b111;
        *self.data = (*self.data & !(0b111 << 4)) | (val as u32) << 4;
    }

    pub fn mps(&mut self) -> u8 {
        return ((*self.data >> 7) & 0b1111) as u8;
    }

    pub fn ams(&mut self) -> u8 {
        return ((*self.data >> 11) & 0b111) as u8;
    }

    pub fn shn(&mut self) -> u8 {
        return ((*self.data >> 14) & 0b11) as u8;
    }

    pub fn iosqes(&self) -> u8 {
        return ((*self.data >> 16) & 0b1111) as u8;
    }

    pub fn set_iosqes(&mut self, mut val: u8) {
        val &= 0b1111;
        *self.data = (*self.data & !(4 << 16)) | (val as u32) << 16;
    }

    pub fn iocqes(&self) -> u8 {
        return ((*self.data >> 20) & 0b1111) as u8;
    }

    pub fn set_iocqes(&mut self, mut val: u8) {
        val &= 0b1111;
        *self.data = (*self.data & !(4 << 20)) | (val as u32) << 20;
    }
}

pub struct NVMeCsts<'a> {
    data: &'a u32,
}

impl<'a> NVMeCsts<'a> {
    pub fn new(data: &'a u32) -> Self {
        return Self { data };
    }

    pub fn rdy(&self) -> bool {
        return 1 == (self.data & 1);
    }
    pub fn cfs(&self) -> bool {
        return 1 == (self.data >> 1) & 1;
    }
    pub fn shst(&self) -> u8 {
        return ((self.data >> 2) & 0b11) as u8;
    }
    pub fn nssro(&self) -> bool {
        return 1 == (self.data >> 4) & 1;
    }

    pub fn pp(&self) -> bool {
        return 1 == (self.data >> 5) & 1;
    }
}

pub struct NVMeAqa<'a> {
    data: &'a mut u32,
}

impl<'a> NVMeAqa<'a> {
    pub fn new(data: &'a mut u32) -> Self {
        return Self { data };
    }

    pub fn set_asqs(&mut self, mut val: u16) {
        val &= 0b1111_1111_1111;
        *self.data = (*self.data & !0b1111_1111_1111) | (val as u32);
    }

    pub fn set_acqs(&mut self, mut val: u16) {
        val &= 0b1111_1111_1111;
        *self.data = (*self.data & !(0b1111_1111_1111 << 16)) | (val as u32) << 16;
    }
}

pub struct NVMeBar {
    physical_address: u64,
    data: *mut c_void,
}

impl NVMeBar {
    pub fn address(&self) -> u64 {
        return self.data as u64;
    }

    pub fn get_physical_address(&self) -> u64 {
        return self.physical_address;
    }

    fn offset_as_u32(&self, offset: usize) -> &'static u32 {
        return unsafe { (self.data.add(offset) as *const u32).as_ref().unwrap() };
    }

    fn offset_as_mut_u32(&mut self, offset: usize) -> &'static mut u32 {
        return unsafe { (self.data.add(offset) as *mut u32).as_mut().unwrap() };
    }

    fn offset_as_u64(&self, offset: usize) -> &'static u64 {
        return unsafe { (self.data.add(offset) as *const u64).as_ref().unwrap() };
    }

    fn offset_as_mut_u64(&mut self, offset: usize) -> &'static mut u64 {
        return unsafe { (self.data.add(offset) as *mut u64).as_mut().unwrap() };
    }

    pub fn new(physical_address: u64, address: *mut c_void) -> Self {
        return Self {
            physical_address,
            data: address,
        };
    }

    pub fn cap(&self) -> NVMeCap<'static> {
        return NVMeCap::new(self.offset_as_u64(0x00));
    }
    pub fn vs(&self) -> NVMeVS<'static> {
        return NVMeVS::new(self.offset_as_u32(0x08));
    }

    pub fn intms(&self) -> u32 {
        return *self.offset_as_u32(0x0C);
    }
    pub fn intmc(&self) -> u32 {
        return *self.offset_as_u32(0x10);
    }

    pub fn clear_intmc(&mut self, bit: u8) {
        *self.offset_as_mut_u32(0x10) |= 1 << bit as u32;
    }

    pub fn cc(&mut self) -> NVMeCC<'static> {
        return NVMeCC::new(self.offset_as_mut_u32(0x14));
    }
    pub fn csts(&self) -> NVMeCsts<'static> {
        return NVMeCsts::new(self.offset_as_u32(0x1C));
    }
    pub fn aqa(&mut self) -> NVMeAqa<'static> {
        return NVMeAqa::new(self.offset_as_mut_u32(0x24));
    }
    pub fn asq(&self) -> u64 {
        *self.offset_as_u64(0x28)
    }

    pub fn set_asq(&mut self, asq: u64) {
        *self.offset_as_mut_u64(0x28) = asq;
    }

    pub fn acq(&self) -> u64 {
        *self.offset_as_u64(0x30)
    }

    pub fn set_acq(&mut self, acq: u64) {
        *self.offset_as_mut_u64(0x30) = acq;
    }

    pub fn tdbl(&self, tail: u32) -> u64 {
        let stride = 4 << self.cap().dstrd();
        return unsafe {
            self.data
                .add(0x1000 + (2 * tail as usize) * stride as usize)
        } as u64;
    }

    pub fn hdbl(&self, head: u32) -> u64 {
        let stride = 4 << self.cap().dstrd();
        return unsafe {
            self.data
                .add(0x1000 + (2 * head as usize + 1) * stride as usize)
        } as u64;
    }
}
