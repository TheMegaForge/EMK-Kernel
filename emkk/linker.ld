ENTRY(_bootstrap_kentry)
OUTPUT_FORMAT(elf64-x86-64)
SECTIONS
{
  . = 0xFFFF800000000000;
  .kentry : ALIGN(0x1000) {
    *(.kentry)
  }
  .text : ALIGN(0x1000) {
    *(.text*)
  }
  .rodata : ALIGN(0x1000) {
    *(.rodata*)
  }
  .data : ALIGN(0x1000) {
    *(.data*)
   }
  .got : ALIGN(0x1000) {
    *(.got*)
  }
  .bss : ALIGN(0x1000) {
    *(.bss*)
  }
  .service_calls : ALIGN(0x1000) {
    *(.service_calls)
  }
  .exception_calls : ALIGN(0x1000) {
    *(.exception_calls)
  }
  .interrupt_service_fn_array : ALIGN(0x1000) {
    *(.interrupt_service_fn_array)
  }
  .host_core : ALIGN(0x1000) {
    *(.host_core)
  }
  .ap_segment : ALIGN(0x1000) {
    *(.ap_segment)
  }
  .context_switch_functions : ALIGN(0x1000) {
    *(.context_switch_functions)
  }
  .ehci_controller : ALIGN(0x1000) {
    *(.ehci_controller)
  }
  __kend = .;
}
