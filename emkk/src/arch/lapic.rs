use core::ffi::c_void;

use crate::{fixed_vaddrs::FIXED_LOCAL_APIC_VIRTUAL_ADDRESS, utils::cpuid};

pub struct LocalApic {
    address: *mut c_void,
    x2apic: bool,
}

pub const IA32_APIC_BASE_MSR: u64 = 0x1B;
pub const MASK: u64 = 0x900;

pub const DELIVERY_MODE_FIXED: u8 = 0;
pub const DELIVERY_MODE_INIT: u8 = 0b101;
pub const DELIVERY_MODE_START_UP: u8 = 0b110;

pub const DESTINATION_MODE_PHYSICAL: u8 = 0;
pub const DESTINATION_MODE_LOGICAL: u8 = 0;

pub const LEVEL_DE_ASSERT: u8 = 0;
pub const LEVEL_ASSERT: u8 = 1;

pub const TRIGGER_MODE_EDGE: u8 = 0;
pub const TRIGGER_MODE_LEVEL: u8 = 1;

pub const NO_SHORTHAND: u8 = 0;
pub const SELF_SHORTHAND: u8 = 1;
pub const ALL_INCLUDING_SELF_SHORTHAND: u8 = 2;
pub const ALL_EXCLUDING_SELF_SHORTHAND: u8 = 3;

impl LocalApic {
    pub fn from_local_core() -> Self {
        let x2apic = cpuid(1, 0).ecx & (1 << 21) != 0;
        return Self {
            address: FIXED_LOCAL_APIC_VIRTUAL_ADDRESS as *mut c_void,
            x2apic,
        };
    }

    pub fn get_address(&self) -> u64 {
        return self.address as u64;
    }

    pub fn reset(&mut self) {
        unsafe {
            ((self.address.add(0x2F0)) as *mut u32).write_volatile(1 << 16); //CMCI
            ((self.address.add(0x350)) as *mut u32).write_volatile(1 << 16); //LINT0
            ((self.address.add(0x360)) as *mut u32).write_volatile(1 << 16); //LINT1
            ((self.address.add(0x370)) as *mut u32).write_volatile(1 << 16); //ERROR
            ((self.address.add(0x340)) as *mut u32).write_volatile(1 << 16); //PERF MON+ COUNTER
            ((self.address.add(0x330)) as *mut u32).write_volatile(1 << 16); //THERMAL SENSOR.
            ((self.address.add(0x320)) as *mut u32).write_volatile(1 << 16); //TIMER
            ((self.address.add(0x0F0)) as *mut u32).write_volatile(0xFF | 1 << 8); //SPURIOUS INTERRUPT WILL HAPPEN ON ISR 0xFF (255) + ENABLES APIC
            ((self.address.add(0x0B0)) as *mut u32).write_volatile(0); //SENDS EOI
        };
    }

    pub fn send_ipi(
        &self,
        destination: u8,
        interrupt_vector: u8,
        delivery_mode: u8,
        destination_mode: u8,
        level: u8,
        trigger_mode: u8,
        short_hand: u8,
    ) {
        let half0: u32 = interrupt_vector as u32
            | (delivery_mode as u32) << 8
            | ((destination_mode as u32) & 0b111) << 11
            | ((level as u32) & 1) << 14
            | ((trigger_mode as u32) & 1) << 15
            | ((short_hand as u32) & 0b11) << 18;

        unsafe {
            (self.address.add(0x310) as *mut u32).write_volatile((destination as u32) << 24);
            (self.address.add(0x300) as *mut u32).write_volatile(half0);
        }
        while unsafe { (self.address.add(0x300) as *const u32).read_volatile() & (1 << 12) != 0 } {}
    }

    pub fn send_startup_ipi(
        &self,
        destination: u8,
        interrupt_vector: u8,
        destination_mode: u8,
        short_hand: u8,
    ) {
        let half0: u32 = interrupt_vector as u32
            | (DELIVERY_MODE_START_UP as u32) << 8
            | ((destination_mode as u32) & 0b111) << 11
            | 0 << 14
            | 0 << 15 // ignored for sipi
            | ((short_hand as u32) & 0b11) << 18;

        unsafe {
            (self.address.add(0x310) as *mut u32).write_volatile((destination as u32) << 24);
            (self.address.add(0x300) as *mut u32).write_volatile(half0);
        }

        while unsafe { (self.address.add(0x300) as *const u32).read_volatile() & (1 << 12) != 0 } {}
    }

    pub fn get_id(&self) -> u32 {
        return unsafe { ((self.address.add(0x20)) as *const u32).read_volatile() };
    }
    pub fn get_version(&self) -> u8 {
        return unsafe { ((self.address.add(0x30)) as *const u8).read_volatile() };
    }

    pub fn send_eoi(&mut self) {
        unsafe { ((self.address.add(0xB0)) as *mut u32).write_volatile(0) };
    }
}
