use core::arch::asm;

#[inline(always)]
pub unsafe fn wrmsr(msr: u64, value: u64) {
    let low: u32 = (value & 0xFFFFFFFF) as u32;
    let high: u32 = (value >> 32) as u32;

    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
            options(nostack, preserves_flags),
        );
    }
}
#[inline(always)]
pub fn rdmsr(msr: u64) -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
        );
    }
    return (high as u64) << 32 | low as u64;
}

#[inline(always)]
pub unsafe fn outb(port: u16, val: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") val,
            options(nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub unsafe fn outw(port: u16, val: u16) {
    unsafe {
        asm!(
            "out {0:x}, {1:x}",
            in(reg) val,
            in(reg) port,
            options(nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub unsafe fn outd(port: u16, val: u32) {
    unsafe {
        asm!(
            "out {0:e}, {1:x}",
            in(reg) val,
            in(reg) port,
            options(nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub unsafe fn inb(port: u16) -> u8 {
    let ret: u8;
    unsafe {
        asm!(
            "inb {1:x}, {0}",
            out(reg_byte) ret,
            in(reg) port,
            options(nostack, preserves_flags)
        );
    }
    return ret;
}

#[inline(always)]
pub unsafe fn inw(port: u16) -> u16 {
    let ret: u16;
    unsafe {
        asm!(
            "inw {1:x}, {0:x}",
            out(reg) ret,
            in(reg) port,
            options(nostack, preserves_flags)
        );
    }
    return ret;
}

#[inline(always)]
pub unsafe fn ind(port: u16) -> u32 {
    let ret: u32;
    unsafe {
        asm!(
            "ind {1:x}, {0:e}",
            out(reg) ret,
            in(reg) port,
            options(nostack, preserves_flags)
        );
    }
    return ret;
}

#[inline(always)]
pub unsafe fn io_wait() {
    unsafe { outb(0x80, 0) };
}

unsafe extern "C" {
    pub fn get_cr3() -> *mut u64;
    pub fn set_cr3(ptr: *mut u64);
}
