section .context_switch_functions alloc exec
[global launch_application]
; rdi = cr3
; rsi = rbp
; rdx = rip
launch_application:

    mov ax, 0x18 | 3 ; user data segment
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax ; loading data segments

    mov rbp, rsi
    mov rsp, rbp ; loading stack

    mov cr3, rdi ; paging

    mov rcx, rdx
    o64 sysret ; starts the real application

    cli
    hlt ; Sanity halt
