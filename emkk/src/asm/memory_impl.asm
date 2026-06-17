
[global memset]
[global memset_word]
[global memset_dword]
[global memset_qword]
[global memcpy]
[global memcpy_qword]
;rdi = ptr
;rsi = value
;rdx = length
memset:
    push rcx
    push rdi
    push rax
    mov rcx, rdx
    mov rax, rsi
    rep stosb
    pop rax
    pop rdi
    pop rcx
    ret

;rdi = ptr
;rsi = value
;rdx = length
memset_word:
    push rcx
    push rdi
    push rax
    mov rcx, rdx
    mov rax, rsi
    rep stosw
    pop rax
    pop rdi
    pop rcx
    ret

;rdi = ptr
;rsi = value
;rdx = length
memset_dword:
    push rcx
    push rdi
    push rax
    mov rcx, rdx
    mov rax, rsi
    rep stosd
    pop rax
    pop rdi
    pop rcx
    ret

;rdi = ptr
;rsi = value
;rdx = length
memset_qword:
    push rcx
    push rdi
    push rax
    mov rcx, rdx
    mov rax, rsi
    rep stosq
    pop rax
    pop rdi
    pop rcx
    ret

;rdi = dst
;rsi = src
;rdx = length
memcpy_qword:
    push rcx
    push rdi
    push rsi
    mov rcx, rdx
    rep movsq
    pop rsi
    pop rdi
    pop rcx
    ret


;rdi = dst
;rsi = src
;rdx = length
memcpy:
    push rcx
    push rdi
    push rsi
    mov rcx, rdx
    rep movsb
    pop rsi
    pop rdi
    pop rcx
    ret
