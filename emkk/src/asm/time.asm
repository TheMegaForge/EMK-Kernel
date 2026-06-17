[global m_Counter]
m_Counter: dq 0


_m_ms_divider: dq 10

;ms = rdi
global sleep0
sleep0:
    push r8
    push r9
    mov r8,[rel m_Counter]

    push rax
    push rdx

    mov rax,rdi
    xor rdx,rdx

    div qword [rel _m_ms_divider]
    mov r9,rax

    pop rax
    pop rdx

   add r8,r9 ; r8 = expected
.CMP:
    hlt ; halts cpu until timer interrupt happens.
    cmp r8,[rel m_Counter]
    jge .CMP

    pop r9
    pop r8
    ret

global current_tick0
current_tick0:
    mov rax, [rel m_Counter]
    ret
