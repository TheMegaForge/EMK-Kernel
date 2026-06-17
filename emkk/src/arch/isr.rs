use crate::{
    error, fixed_vaddrs::FIXED_PROCESSOR_VIRTUAL_ADDRESS, hal::print::Module,
    multithreading::processors::Processor,
};
use core::arch::asm;
#[repr(C)]
pub struct ISRRegisters {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub isr_number: u64,
    pub error_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
}

pub fn invalid_isr(registers: &ISRRegisters) {
    let mut invalid_isr_module = Module::new("ISR");
    error!(
        invalid_isr_module,
        "Invalid ISR #{} (IDT #{}) Called at RIP = 0x{:x}\n",
        registers.isr_number - 32,
        registers.isr_number,
        registers.rip
    );
    error!(invalid_isr_module, "Error Code = {}", registers.error_code);
    unsafe { asm!("cli;hlt") };
    loop {}
}

//index 0 = isr32
#[unsafe(link_section = ".interrupt_service_fn_array")]
pub static mut HOST_CORE_ISR_CALLS: [fn(&ISRRegisters); 256 - 32] = [invalid_isr; 256 - 32];

#[unsafe(no_mangle)]
#[unsafe(link_section = ".service_calls")]
fn raw_call(registers: &ISRRegisters) {
    let processor = FIXED_PROCESSOR_VIRTUAL_ADDRESS as *const Processor;
    let isr: *mut [fn(&ISRRegisters)] = unsafe { (*processor).isr };
    let function: fn(&ISRRegisters) = unsafe { (*isr)[registers.isr_number as usize - 32] };
    (function)(registers);
}
