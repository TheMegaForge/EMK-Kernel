use crate::{arch::isr::ISRRegisters, error, hal::print::Module};

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_divide(isr: &ISRRegisters) -> ! {
    _rce_common("Division Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_debug(isr: &ISRRegisters) -> ! {
    _rce_common("Debug Interrupt", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_nmi(isr: &ISRRegisters) -> ! {
    _rce_common("\"Non Maskable Interrupt\" Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_breakpoint(isr: &ISRRegisters) -> ! {
    _rce_common("Breakpoint Interrupt", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_overflow(isr: &ISRRegisters) -> ! {
    _rce_common("Overflow Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_bound(isr: &ISRRegisters) -> ! {
    _rce_common("\"Bound Exceeded\" Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_invalid_opcode(isr: &ISRRegisters) -> ! {
    _rce_common("\"Invalid Opcode\" Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_device_not_available(isr: &ISRRegisters) -> ! {
    _rce_common("\"Device Not Available\" Error", isr.rip)
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_double_fault(isr: &ISRRegisters) -> ! {
    _rce_common("Double Fault", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_coprocessor_overrun(isr: &ISRRegisters) -> ! {
    _rce_common("\"Coprocessor Overrun\" Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_invalid_tss(isr: &ISRRegisters) -> ! {
    _rce_common("\"Invalid Tss\" Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_segment_not_present(isr: &ISRRegisters) -> ! {
    _rce_common("\"Segment Not Present\" Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_stack_segment(isr: &ISRRegisters) -> ! {
    _rce_common("Stack Segment Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_general_protection(isr: &ISRRegisters) -> ! {
    _rce_common("General Protection Fault", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_page_fault(isr: &ISRRegisters) -> ! {
    _rce_common("Page Fault", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_x87_floating(isr: &ISRRegisters) -> ! {
    _rce_common("x87 Floating Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_alignment(isr: &ISRRegisters) -> ! {
    _rce_common("Alignment Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_machine_check(isr: &ISRRegisters) -> ! {
    _rce_common("Machine Check Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_simd_floating(isr: &ISRRegisters) -> ! {
    _rce_common("Simd Floating Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_virtualization(isr: &ISRRegisters) -> ! {
    _rce_common("Virtualization Error", isr.rip);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".exception_calls")]
fn _rce_control_protection(isr: &ISRRegisters) -> ! {
    _rce_common("Control Protection Error", isr.rip);
}

fn _rce_common(exception_name: &'static str, location: u64) -> ! {
    let mut module = Module::new("x86_64/Exception");
    error!(
        module,
        "CRITICAL: --- {} happended at location 0x{:x} \n", exception_name, location
    );

    loop {}
}
