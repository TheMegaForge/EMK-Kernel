[global load_gdt]
[global load_ltr]
[global load_idt]
[global get_gdt_base]
[global get_idt_base]
[global get_cr3]
[global set_cr3]
[global popcount]

;rdi = ptr
load_gdt:
   lgdt [rdi]
   mov ax, 0
   mov fs, ax
   mov gs, ax
   mov ax, 0x10
   mov ds, ax
   mov es, ax
   mov ss, ax
   pop rdi
   mov rax, 0x08
   push rax
   push rdi
   retfq
load_ltr:
    mov ax, 0x38
    ltr ax
    ret
load_idt:
    lidt [rdi]
    ret

exhaust: times 10 db 0

get_gdt_base:
    sgdt [rel exhaust]
    mov rax, [rel exhaust + 2]
    ret
get_idt_base:
    sidt [rel exhaust]
    mov rax, [rel exhaust + 2]
    ret

get_cr3:
    mov rax, cr3
    ret
set_cr3:
    mov rax, rdi
    mov cr3, rax
    ret
;rdi = val
popcount:
    popcnt eax, edi
    ret
