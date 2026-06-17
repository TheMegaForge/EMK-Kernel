use core::ffi::c_void;

use crate::utils::memory::memcpy;

pub trait Reader {
    fn read_u8(&mut self) -> u8;
    fn read_u16(&mut self) -> u16;
    fn read_u32(&mut self) -> u32;
    fn read_u64(&mut self) -> u64;
    fn read_bytes(&mut self, buffer: &mut [u8]) -> bool;

    fn skip(&mut self, len: u32) -> bool;
    fn go_back(&mut self, len: u32) -> bool;

    fn peek_u8(&self, offset: u32) -> u8;

    fn current(&self) -> *const c_void;
}

pub struct BufferedReader {
    buf: *const c_void,
    len: u32,
    pos: u32,
}

impl BufferedReader {
    pub fn new(buf: *const c_void, len: u32) -> Self {
        return Self { buf, len, pos: 0 };
    }
    pub fn remaining_bytes(&self) -> u32 {
        return self.len - self.pos;
    }
}

impl Reader for BufferedReader {
    fn current(&self) -> *const c_void {
        return unsafe { self.buf.add(self.pos as usize) };
    }

    fn read_u8(&mut self) -> u8 {
        let m = unsafe { self.buf.add(self.pos as usize) };
        self.pos += 1;
        return unsafe { (m as *const u8).read_unaligned() };
    }
    fn read_u16(&mut self) -> u16 {
        let m = unsafe { self.buf.add(self.pos as usize) };
        self.pos += 2;
        return unsafe { (m as *const u16).read_unaligned() };
    }
    fn read_u32(&mut self) -> u32 {
        let m = unsafe { self.buf.add(self.pos as usize) };
        self.pos += 4;
        return unsafe { (m as *const u32).read_unaligned() };
    }
    fn read_u64(&mut self) -> u64 {
        let m = unsafe { self.buf.add(self.pos as usize) };
        self.pos += 8;
        return unsafe { (m as *const u64).read_unaligned() };
    }
    fn read_bytes(&mut self, buffer: &mut [u8]) -> bool {
        if self.pos + buffer.len() as u32 > self.len {
            return false;
        }
        let m = unsafe { self.buf.add(self.pos as usize) };
        unsafe { memcpy(buffer.as_mut_ptr() as *mut c_void, m, buffer.len() as u32) };
        self.pos += buffer.len() as u32;
        return true;
    }
    fn skip(&mut self, len: u32) -> bool {
        if self.pos + len > self.len {
            return false;
        }
        self.pos += len;
        return true;
    }
    fn go_back(&mut self, len: u32) -> bool {
        if (self.pos as i64) - (len as i64) < 0 {
            return false;
        }
        self.pos -= len;
        return true;
    }

    fn peek_u8(&self, offset: u32) -> u8 {
        return unsafe {
            (self.buf.add((self.pos + offset) as usize) as *const u8).read_unaligned()
        };
    }
}
