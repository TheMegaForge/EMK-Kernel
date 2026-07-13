[global load_gdt]
[global load_ltr]
[global load_idt]
[global get_gdt_base]
[global get_idt_base]
[global get_cr3]
[global set_cr3]
[global popcount]

[global inb]
[global inw]
[global ind]
[global outb]
[global outw]
[global outd]
inb:
    push rdx
    mov dx, di
    in al,dx
    pop rdx
    ret
inw:
    push rdx
    mov dx, di
    in ax, dx
    pop rdx
    ret
ind:
    push rdx
    mov dx, di
    in eax, dx
    pop rdx
    ret
outb:
    push rdx
    mov dx, di
    mov al, sil
    out dx, al
    pop rdx
    ret
outw:
    push rdx
    mov dx, di
    mov ax, si
    out dx, ax
    pop rdx
    ret
outd:
    push rdx
    mov dx, di
    mov eax, esi
    out dx, eax
    pop rdx
    ret

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
