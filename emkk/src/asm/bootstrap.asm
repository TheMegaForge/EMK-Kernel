[global _bootstrap_kentry]
[global _ap_trampoline]
[global _ap_trampoline_end]
[global _ap_bootup0]
[global _ap_bootup0_end]
[global _ap_bootup1]
[global _ap_bootup1_end]
[extern kentry]
section .kentry alloc exec

__tmp_font: dq 0
__tmp_rsdp: dq 0
__tmp_allocations: dq 0
__tmp_kernel_stack: dq 0

_bootstrap_kentry:
    xor rax,rax

    mov rax, [rsp + 32]
    mov [rel __tmp_kernel_stack], rax


    mov rax, [rsp + 24]
    mov [rel __tmp_allocations], rax


    mov rax, [rsp + 16]
    mov [rel __tmp_rsdp], rax


    mov rax, [rsp + 8]
    mov [rel __tmp_font], rax

    mov rbp, [rsp + 32]
    mov rsp, rbp; New stack

    mov rax, [rel __tmp_kernel_stack]
    mov [rsp + 32], rax

    mov rax, [rel __tmp_allocations]
    mov [rsp + 24], rax


    mov rax, [rel __tmp_rsdp]
    mov [rsp + 16], rax

    mov rax, [rel __tmp_font]
    mov [rsp + 8], rax

    jmp kentry

section .text
[bits 16]
; Expand this!
_ap_trampoline:
    jmp 0x0:0x8005
    cli

    lgdt [0x9080]

    mov eax, cr0
    or eax,1                          ; enables protected mode
    mov cr0, eax

    jmp 0x28:0xA000
_ap_trampoline_end: db 0
[bits 32]
_ap_bootup0:
    mov eax, [0x9000]
    mov cr3, eax


    mov eax, cr4
    or eax, 1<<4 | 1<<5
    mov cr4, eax


    mov ecx, 0xC0000080               ; Read from the EFER MSR.
    rdmsr
    or eax, 0x00000100                ; Set the LME bit.
    wrmsr

    mov eax, cr0
    and eax, ~(1<<30)
    or eax,1<<31                        ; enables paging
    mov cr0, eax
    jmp 0x08:0xA100
    hlt
_ap_bootup0_end: db 0
[bits 64]
_ap_bootup1:

    mov ax,0
    mov es, ax
    mov gs, ax
    mov ax, 0x10
    mov ds, ax
    mov ss, ax

    mov rbp, [0x9008]
    mov rsp, rbp

    mov rax, [0x9010]

    mov rsi, [0x9018]
    mov rdi, [0x9028]
    mov rcx, [0x9030]
    mov rdx, [0x9038]
    mov r8, [0x9040]
    jmp rax
_ap_bootup1_end: db 0
