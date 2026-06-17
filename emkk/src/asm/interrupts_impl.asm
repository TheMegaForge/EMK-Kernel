%macro exception_error_code 1
    push 0
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov rdi,rsp
    call %1
    iretq
%endmacro

%macro exception_no_error_code 1
    push 0
    push 0
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov rdi,rsp
    call %1
    iretq
%endmacro

[extern _rce_divide]
[extern _rce_debug]
[extern _rce_nmi]
[extern _rce_breakpoint]
[extern _rce_overflow]
[extern _rce_bound]
[extern _rce_invalid_opcode]
[extern _rce_device_not_available]
[extern _rce_double_fault]
[extern _rce_coprocessor_overrun]
[extern _rce_invalid_tss]
[extern _rce_segment_not_present]
[extern _rce_stack_segment]
[extern _rce_general_protection]
[extern _rce_page_fault]
[extern _rce_x87_floating]
[extern _rce_alignment]
[extern _rce_machine_check]
[extern _rce_simd_floating]
[extern _rce_virtualization]
[extern _rce_control_protection]

[global rce_divide]
[global rce_debug]
[global rce_nmi]
[global rce_breakpoint]
[global rce_overflow]
[global rce_bound]
[global rce_invalid_opcode]
[global rce_device_not_available]
[global rce_double_fault]
[global rce_coprocessor_overrun]
[global rce_invalid_tss]
[global rce_segment_not_present]
[global rce_stack_segment]
[global rce_general_protection]
[global rce_page_fault]
[global rce_x87_floating]
[global rce_alignment]
[global rce_machine_check]
[global rce_simd_floating]
[global rce_virtualization]
[global rce_control_protection]

section .exception_calls alloc exec

rce_divide:
    exception_no_error_code _rce_divide
rce_debug:
    exception_no_error_code _rce_debug
rce_nmi:
    exception_no_error_code _rce_nmi
rce_breakpoint:
    exception_no_error_code _rce_breakpoint
rce_overflow:
    exception_no_error_code _rce_overflow
rce_bound:
    exception_no_error_code _rce_bound
rce_invalid_opcode:
    exception_no_error_code _rce_invalid_opcode
rce_device_not_available:
    exception_no_error_code _rce_device_not_available
rce_double_fault:
    exception_error_code _rce_double_fault
rce_coprocessor_overrun:
    exception_no_error_code _rce_coprocessor_overrun
rce_invalid_tss:
    exception_error_code _rce_invalid_tss
rce_segment_not_present:
    exception_error_code _rce_segment_not_present
rce_stack_segment:
    exception_error_code _rce_stack_segment
rce_general_protection:
    exception_error_code _rce_general_protection
rce_page_fault:
    exception_error_code _rce_page_fault
rce_x87_floating:
    exception_no_error_code _rce_x87_floating
rce_alignment:
    exception_error_code _rce_alignment
rce_machine_check:
    exception_no_error_code _rce_machine_check
rce_simd_floating:
    exception_no_error_code _rce_simd_floating
rce_virtualization:
    exception_no_error_code _rce_virtualization
rce_control_protection:
    exception_error_code _rce_control_protection

section .service_calls alloc exec



exhaust: dq 0
tmp_isr: dq 0
[extern raw_call]
call_isr_handler:
    push 0
    push qword [rel tmp_isr]
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov rdi,rsp
    call raw_call
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    pop qword [rel exhaust]
    pop qword [rel exhaust]
    iretq
[global isr32]
[global isr33]
[global isr34]
[global isr35]
[global isr36]
[global isr37]
[global isr38]
[global isr39]
[global isr40]
[global isr41]
[global isr42]
[global isr43]
[global isr44]
[global isr45]
[global isr46]
[global isr47]
[global isr48]
[global isr49]
[global isr50]
[global isr51]
[global isr52]
[global isr53]
[global isr54]
[global isr55]
[global isr56]
[global isr57]
[global isr58]
[global isr59]
[global isr60]
[global isr61]
[global isr62]
[global isr63]
[global isr64]
[global isr65]
[global isr66]
[global isr67]
[global isr68]
[global isr69]
[global isr70]
[global isr71]
[global isr72]
[global isr73]
[global isr74]
[global isr75]
[global isr76]
[global isr77]
[global isr78]
[global isr79]
[global isr80]
[global isr81]
[global isr82]
[global isr83]
[global isr84]
[global isr85]
[global isr86]
[global isr87]
[global isr88]
[global isr89]
[global isr90]
[global isr91]
[global isr92]
[global isr93]
[global isr94]
[global isr95]
[global isr96]
[global isr97]
[global isr98]
[global isr99]
[global isr100]
[global isr101]
[global isr102]
[global isr103]
[global isr104]
[global isr105]
[global isr106]
[global isr107]
[global isr108]
[global isr109]
[global isr110]
[global isr111]
[global isr112]
[global isr113]
[global isr114]
[global isr115]
[global isr116]
[global isr117]
[global isr118]
[global isr119]
[global isr120]
[global isr121]
[global isr122]
[global isr123]
[global isr124]
[global isr125]
[global isr126]
[global isr127]
[global isr128]
[global isr129]
[global isr130]
[global isr131]
[global isr132]
[global isr133]
[global isr134]
[global isr135]
[global isr136]
[global isr137]
[global isr138]
[global isr139]
[global isr140]
[global isr141]
[global isr142]
[global isr143]
[global isr144]
[global isr145]
[global isr146]
[global isr147]
[global isr148]
[global isr149]
[global isr150]
[global isr151]
[global isr152]
[global isr153]
[global isr154]
[global isr155]
[global isr156]
[global isr157]
[global isr158]
[global isr159]
[global isr160]
[global isr161]
[global isr162]
[global isr163]
[global isr164]
[global isr165]
[global isr166]
[global isr167]
[global isr168]
[global isr169]
[global isr170]
[global isr171]
[global isr172]
[global isr173]
[global isr174]
[global isr175]
[global isr176]
[global isr177]
[global isr178]
[global isr179]
[global isr180]
[global isr181]
[global isr182]
[global isr183]
[global isr184]
[global isr185]
[global isr186]
[global isr187]
[global isr188]
[global isr189]
[global isr190]
[global isr191]
[global isr192]
[global isr193]
[global isr194]
[global isr195]
[global isr196]
[global isr197]
[global isr198]
[global isr199]
[global isr200]
[global isr201]
[global isr202]
[global isr203]
[global isr204]
[global isr205]
[global isr206]
[global isr207]
[global isr208]
[global isr209]
[global isr210]
[global isr211]
[global isr212]
[global isr213]
[global isr214]
[global isr215]
[global isr216]
[global isr217]
[global isr218]
[global isr219]
[global isr220]
[global isr221]
[global isr222]
[global isr223]
[global isr224]
[global isr225]
[global isr226]
[global isr227]
[global isr228]
[global isr229]
[global isr230]
[global isr231]
[global isr232]
[global isr233]
[global isr234]
[global isr235]
[global isr236]
[global isr237]
[global isr238]
[global isr239]
[global isr240]
[global isr241]
[global isr242]
[global isr243]
[global isr244]
[global isr245]
[global isr246]
[global isr247]
[global isr248]
[global isr249]
[global isr250]
[global isr251]
[global isr252]
[global isr253]
[global isr254]
[global isr255]

isr32:
 mov qword [rel tmp_isr],32
 jmp call_isr_handler
isr33:
 mov qword [rel tmp_isr],33
 jmp call_isr_handler
isr34:
 mov qword [rel tmp_isr],34
 jmp call_isr_handler
isr35:
 mov qword [rel tmp_isr],35
 jmp call_isr_handler
isr36:
 mov qword [rel tmp_isr],36
 jmp call_isr_handler
isr37:
 mov qword [rel tmp_isr],37
 jmp call_isr_handler
isr38:
 mov qword [rel tmp_isr],38
 jmp call_isr_handler
isr39:
 mov qword [rel tmp_isr],39
 jmp call_isr_handler
isr40:
 mov qword [rel tmp_isr],40
 jmp call_isr_handler
isr41:
 mov qword [rel tmp_isr],41
 jmp call_isr_handler
isr42:
 mov qword [rel tmp_isr],42
 jmp call_isr_handler
isr43:
 mov qword [rel tmp_isr],43
 jmp call_isr_handler
isr44:
 mov qword [rel tmp_isr],44
 jmp call_isr_handler
isr45:
 mov qword [rel tmp_isr],45
 jmp call_isr_handler
isr46:
 mov qword [rel tmp_isr],46
 jmp call_isr_handler
isr47:
 mov qword [rel tmp_isr],47
 jmp call_isr_handler
isr48:
 mov qword [rel tmp_isr],48
 jmp call_isr_handler
isr49:
 mov qword [rel tmp_isr],49
 jmp call_isr_handler
isr50:
 mov qword [rel tmp_isr],50
 jmp call_isr_handler
isr51:
 mov qword [rel tmp_isr],51
 jmp call_isr_handler
isr52:
 mov qword [rel tmp_isr],52
 jmp call_isr_handler
isr53:
 mov qword [rel tmp_isr],53
 jmp call_isr_handler
isr54:
 mov qword [rel tmp_isr],54
 jmp call_isr_handler
isr55:
 mov qword [rel tmp_isr],55
 jmp call_isr_handler
isr56:
 mov qword [rel tmp_isr],56
 jmp call_isr_handler
isr57:
 mov qword [rel tmp_isr],57
 jmp call_isr_handler
isr58:
 mov qword [rel tmp_isr],58
 jmp call_isr_handler
isr59:
 mov qword [rel tmp_isr],59
 jmp call_isr_handler
isr60:
 mov qword [rel tmp_isr],60
 jmp call_isr_handler
isr61:
 mov qword [rel tmp_isr],61
 jmp call_isr_handler
isr62:
 mov qword [rel tmp_isr],62
 jmp call_isr_handler
isr63:
 mov qword [rel tmp_isr],63
 jmp call_isr_handler
isr64:
 mov qword [rel tmp_isr],64
 jmp call_isr_handler
isr65:
 mov qword [rel tmp_isr],65
 jmp call_isr_handler
isr66:
 mov qword [rel tmp_isr],66
 jmp call_isr_handler
isr67:
 mov qword [rel tmp_isr],67
 jmp call_isr_handler
isr68:
 mov qword [rel tmp_isr],68
 jmp call_isr_handler
isr69:
 mov qword [rel tmp_isr],69
 jmp call_isr_handler
isr70:
 mov qword [rel tmp_isr],70
 jmp call_isr_handler
isr71:
 mov qword [rel tmp_isr],71
 jmp call_isr_handler
isr72:
 mov qword [rel tmp_isr],72
 jmp call_isr_handler
isr73:
 mov qword [rel tmp_isr],73
 jmp call_isr_handler
isr74:
 mov qword [rel tmp_isr],74
 jmp call_isr_handler
isr75:
 mov qword [rel tmp_isr],75
 jmp call_isr_handler
isr76:
 mov qword [rel tmp_isr],76
 jmp call_isr_handler
isr77:
 mov qword [rel tmp_isr],77
 jmp call_isr_handler
isr78:
 mov qword [rel tmp_isr],78
 jmp call_isr_handler
isr79:
 mov qword [rel tmp_isr],79
 jmp call_isr_handler
isr80:
 mov qword [rel tmp_isr],80
 jmp call_isr_handler
isr81:
 mov qword [rel tmp_isr],81
 jmp call_isr_handler
isr82:
 mov qword [rel tmp_isr],82
 jmp call_isr_handler
isr83:
 mov qword [rel tmp_isr],83
 jmp call_isr_handler
isr84:
 mov qword [rel tmp_isr],84
 jmp call_isr_handler
isr85:
 mov qword [rel tmp_isr],85
 jmp call_isr_handler
isr86:
 mov qword [rel tmp_isr],86
 jmp call_isr_handler
isr87:
 mov qword [rel tmp_isr],87
 jmp call_isr_handler
isr88:
 mov qword [rel tmp_isr],88
 jmp call_isr_handler
isr89:
 mov qword [rel tmp_isr],89
 jmp call_isr_handler
isr90:
 mov qword [rel tmp_isr],90
 jmp call_isr_handler
isr91:
 mov qword [rel tmp_isr],91
 jmp call_isr_handler
isr92:
 mov qword [rel tmp_isr],92
 jmp call_isr_handler
isr93:
 mov qword [rel tmp_isr],93
 jmp call_isr_handler
isr94:
 mov qword [rel tmp_isr],94
 jmp call_isr_handler
isr95:
 mov qword [rel tmp_isr],95
 jmp call_isr_handler
isr96:
 mov qword [rel tmp_isr],96
 jmp call_isr_handler
isr97:
 mov qword [rel tmp_isr],97
 jmp call_isr_handler
isr98:
 mov qword [rel tmp_isr],98
 jmp call_isr_handler
isr99:
 mov qword [rel tmp_isr],99
 jmp call_isr_handler
isr100:
 mov qword [rel tmp_isr],100
 jmp call_isr_handler
isr101:
 mov qword [rel tmp_isr],101
 jmp call_isr_handler
isr102:
 mov qword [rel tmp_isr],102
 jmp call_isr_handler
isr103:
 mov qword [rel tmp_isr],103
 jmp call_isr_handler
isr104:
 mov qword [rel tmp_isr],104
 jmp call_isr_handler
isr105:
 mov qword [rel tmp_isr],105
 jmp call_isr_handler
isr106:
 mov qword [rel tmp_isr],106
 jmp call_isr_handler
isr107:
 mov qword [rel tmp_isr],107
 jmp call_isr_handler
isr108:
 mov qword [rel tmp_isr],108
 jmp call_isr_handler
isr109:
 mov qword [rel tmp_isr],109
 jmp call_isr_handler
isr110:
 mov qword [rel tmp_isr],110
 jmp call_isr_handler
isr111:
 mov qword [rel tmp_isr],111
 jmp call_isr_handler
isr112:
 mov qword [rel tmp_isr],112
 jmp call_isr_handler
isr113:
 mov qword [rel tmp_isr],113
 jmp call_isr_handler
isr114:
 mov qword [rel tmp_isr],114
 jmp call_isr_handler
isr115:
 mov qword [rel tmp_isr],115
 jmp call_isr_handler
isr116:
 mov qword [rel tmp_isr],116
 jmp call_isr_handler
isr117:
 mov qword [rel tmp_isr],117
 jmp call_isr_handler
isr118:
 mov qword [rel tmp_isr],118
 jmp call_isr_handler
isr119:
 mov qword [rel tmp_isr],119
 jmp call_isr_handler
isr120:
 mov qword [rel tmp_isr],120
 jmp call_isr_handler
isr121:
 mov qword [rel tmp_isr],121
 jmp call_isr_handler
isr122:
 mov qword [rel tmp_isr],122
 jmp call_isr_handler
isr123:
 mov qword [rel tmp_isr],123
 jmp call_isr_handler
isr124:
 mov qword [rel tmp_isr],124
 jmp call_isr_handler
isr125:
 mov qword [rel tmp_isr],125
 jmp call_isr_handler
isr126:
 mov qword [rel tmp_isr],126
 jmp call_isr_handler
isr127:
 mov qword [rel tmp_isr],127
 jmp call_isr_handler
isr128:
 mov qword [rel tmp_isr],128
 jmp call_isr_handler
isr129:
 mov qword [rel tmp_isr],129
 jmp call_isr_handler
isr130:
 mov qword [rel tmp_isr],130
 jmp call_isr_handler
isr131:
 mov qword [rel tmp_isr],131
 jmp call_isr_handler
isr132:
 mov qword [rel tmp_isr],132
 jmp call_isr_handler
isr133:
 mov qword [rel tmp_isr],133
 jmp call_isr_handler
isr134:
 mov qword [rel tmp_isr],134
 jmp call_isr_handler
isr135:
 mov qword [rel tmp_isr],135
 jmp call_isr_handler
isr136:
 mov qword [rel tmp_isr],136
 jmp call_isr_handler
isr137:
 mov qword [rel tmp_isr],137
 jmp call_isr_handler
isr138:
 mov qword [rel tmp_isr],138
 jmp call_isr_handler
isr139:
 mov qword [rel tmp_isr],139
 jmp call_isr_handler
isr140:
 mov qword [rel tmp_isr],140
 jmp call_isr_handler
isr141:
 mov qword [rel tmp_isr],141
 jmp call_isr_handler
isr142:
 mov qword [rel tmp_isr],142
 jmp call_isr_handler
isr143:
 mov qword [rel tmp_isr],143
 jmp call_isr_handler
isr144:
 mov qword [rel tmp_isr],144
 jmp call_isr_handler
isr145:
 mov qword [rel tmp_isr],145
 jmp call_isr_handler
isr146:
 mov qword [rel tmp_isr],146
 jmp call_isr_handler
isr147:
 mov qword [rel tmp_isr],147
 jmp call_isr_handler
isr148:
 mov qword [rel tmp_isr],148
 jmp call_isr_handler
isr149:
 mov qword [rel tmp_isr],149
 jmp call_isr_handler
isr150:
 mov qword [rel tmp_isr],150
 jmp call_isr_handler
isr151:
 mov qword [rel tmp_isr],151
 jmp call_isr_handler
isr152:
 mov qword [rel tmp_isr],152
 jmp call_isr_handler
isr153:
 mov qword [rel tmp_isr],153
 jmp call_isr_handler
isr154:
 mov qword [rel tmp_isr],154
 jmp call_isr_handler
isr155:
 mov qword [rel tmp_isr],155
 jmp call_isr_handler
isr156:
 mov qword [rel tmp_isr],156
 jmp call_isr_handler
isr157:
 mov qword [rel tmp_isr],157
 jmp call_isr_handler
isr158:
 mov qword [rel tmp_isr],158
 jmp call_isr_handler
isr159:
 mov qword [rel tmp_isr],159
 jmp call_isr_handler
isr160:
 mov qword [rel tmp_isr],160
 jmp call_isr_handler
isr161:
 mov qword [rel tmp_isr],161
 jmp call_isr_handler
isr162:
 mov qword [rel tmp_isr],162
 jmp call_isr_handler
isr163:
 mov qword [rel tmp_isr],163
 jmp call_isr_handler
isr164:
 mov qword [rel tmp_isr],164
 jmp call_isr_handler
isr165:
 mov qword [rel tmp_isr],165
 jmp call_isr_handler
isr166:
 mov qword [rel tmp_isr],166
 jmp call_isr_handler
isr167:
 mov qword [rel tmp_isr],167
 jmp call_isr_handler
isr168:
 mov qword [rel tmp_isr],168
 jmp call_isr_handler
isr169:
 mov qword [rel tmp_isr],169
 jmp call_isr_handler
isr170:
 mov qword [rel tmp_isr],170
 jmp call_isr_handler
isr171:
 mov qword [rel tmp_isr],171
 jmp call_isr_handler
isr172:
 mov qword [rel tmp_isr],172
 jmp call_isr_handler
isr173:
 mov qword [rel tmp_isr],173
 jmp call_isr_handler
isr174:
 mov qword [rel tmp_isr],174
 jmp call_isr_handler
isr175:
 mov qword [rel tmp_isr],175
 jmp call_isr_handler
isr176:
 mov qword [rel tmp_isr],176
 jmp call_isr_handler
isr177:
 mov qword [rel tmp_isr],177
 jmp call_isr_handler
isr178:
 mov qword [rel tmp_isr],178
 jmp call_isr_handler
isr179:
 mov qword [rel tmp_isr],179
 jmp call_isr_handler
isr180:
 mov qword [rel tmp_isr],180
 jmp call_isr_handler
isr181:
 mov qword [rel tmp_isr],181
 jmp call_isr_handler
isr182:
 mov qword [rel tmp_isr],182
 jmp call_isr_handler
isr183:
 mov qword [rel tmp_isr],183
 jmp call_isr_handler
isr184:
 mov qword [rel tmp_isr],184
 jmp call_isr_handler
isr185:
 mov qword [rel tmp_isr],185
 jmp call_isr_handler
isr186:
 mov qword [rel tmp_isr],186
 jmp call_isr_handler
isr187:
 mov qword [rel tmp_isr],187
 jmp call_isr_handler
isr188:
 mov qword [rel tmp_isr],188
 jmp call_isr_handler
isr189:
 mov qword [rel tmp_isr],189
 jmp call_isr_handler
isr190:
 mov qword [rel tmp_isr],190
 jmp call_isr_handler
isr191:
 mov qword [rel tmp_isr],191
 jmp call_isr_handler
isr192:
 mov qword [rel tmp_isr],192
 jmp call_isr_handler
isr193:
 mov qword [rel tmp_isr],193
 jmp call_isr_handler
isr194:
 mov qword [rel tmp_isr],194
 jmp call_isr_handler
isr195:
 mov qword [rel tmp_isr],195
 jmp call_isr_handler
isr196:
 mov qword [rel tmp_isr],196
 jmp call_isr_handler
isr197:
 mov qword [rel tmp_isr],197
 jmp call_isr_handler
isr198:
 mov qword [rel tmp_isr],198
 jmp call_isr_handler
isr199:
 mov qword [rel tmp_isr],199
 jmp call_isr_handler
isr200:
 mov qword [rel tmp_isr],200
 jmp call_isr_handler
isr201:
 mov qword [rel tmp_isr],201
 jmp call_isr_handler
isr202:
 mov qword [rel tmp_isr],202
 jmp call_isr_handler
isr203:
 mov qword [rel tmp_isr],203
 jmp call_isr_handler
isr204:
 mov qword [rel tmp_isr],204
 jmp call_isr_handler
isr205:
 mov qword [rel tmp_isr],205
 jmp call_isr_handler
isr206:
 mov qword [rel tmp_isr],206
 jmp call_isr_handler
isr207:
 mov qword [rel tmp_isr],207
 jmp call_isr_handler
isr208:
 mov qword [rel tmp_isr],208
 jmp call_isr_handler
isr209:
 mov qword [rel tmp_isr],209
 jmp call_isr_handler
isr210:
 mov qword [rel tmp_isr],210
 jmp call_isr_handler
isr211:
 mov qword [rel tmp_isr],211
 jmp call_isr_handler
isr212:
 mov qword [rel tmp_isr],212
 jmp call_isr_handler
isr213:
 mov qword [rel tmp_isr],213
 jmp call_isr_handler
isr214:
 mov qword [rel tmp_isr],214
 jmp call_isr_handler
isr215:
 mov qword [rel tmp_isr],215
 jmp call_isr_handler
isr216:
 mov qword [rel tmp_isr],216
 jmp call_isr_handler
isr217:
 mov qword [rel tmp_isr],217
 jmp call_isr_handler
isr218:
 mov qword [rel tmp_isr],218
 jmp call_isr_handler
isr219:
 mov qword [rel tmp_isr],219
 jmp call_isr_handler
isr220:
 mov qword [rel tmp_isr],220
 jmp call_isr_handler
isr221:
 mov qword [rel tmp_isr],221
 jmp call_isr_handler
isr222:
 mov qword [rel tmp_isr],222
 jmp call_isr_handler
isr223:
 mov qword [rel tmp_isr],223
 jmp call_isr_handler
isr224:
 mov qword [rel tmp_isr],224
 jmp call_isr_handler
isr225:
 mov qword [rel tmp_isr],225
 jmp call_isr_handler
isr226:
 mov qword [rel tmp_isr],226
 jmp call_isr_handler
isr227:
 mov qword [rel tmp_isr],227
 jmp call_isr_handler
isr228:
 mov qword [rel tmp_isr],228
 jmp call_isr_handler
isr229:
 mov qword [rel tmp_isr],229
 jmp call_isr_handler
isr230:
 mov qword [rel tmp_isr],230
 jmp call_isr_handler
isr231:
 mov qword [rel tmp_isr],231
 jmp call_isr_handler
isr232:
 mov qword [rel tmp_isr],232
 jmp call_isr_handler
isr233:
 mov qword [rel tmp_isr],233
 jmp call_isr_handler
isr234:
 mov qword [rel tmp_isr],234
 jmp call_isr_handler
isr235:
 mov qword [rel tmp_isr],235
 jmp call_isr_handler
isr236:
 mov qword [rel tmp_isr],236
 jmp call_isr_handler
isr237:
 mov qword [rel tmp_isr],237
 jmp call_isr_handler
isr238:
 mov qword [rel tmp_isr],238
 jmp call_isr_handler
isr239:
 mov qword [rel tmp_isr],239
 jmp call_isr_handler
isr240:
 mov qword [rel tmp_isr],240
 jmp call_isr_handler
isr241:
 mov qword [rel tmp_isr],241
 jmp call_isr_handler
isr242:
 mov qword [rel tmp_isr],242
 jmp call_isr_handler
isr243:
 mov qword [rel tmp_isr],243
 jmp call_isr_handler
isr244:
 mov qword [rel tmp_isr],244
 jmp call_isr_handler
isr245:
 mov qword [rel tmp_isr],245
 jmp call_isr_handler
isr246:
 mov qword [rel tmp_isr],246
 jmp call_isr_handler
isr247:
 mov qword [rel tmp_isr],247
 jmp call_isr_handler
isr248:
 mov qword [rel tmp_isr],248
 jmp call_isr_handler
isr249:
 mov qword [rel tmp_isr],249
 jmp call_isr_handler
isr250:
 mov qword [rel tmp_isr],250
 jmp call_isr_handler
isr251:
 mov qword [rel tmp_isr],251
 jmp call_isr_handler
isr252:
 mov qword [rel tmp_isr],252
 jmp call_isr_handler
isr253:
 mov qword [rel tmp_isr],253
 jmp call_isr_handler
isr254:
 mov qword [rel tmp_isr],254
 jmp call_isr_handler
isr255:
 mov qword [rel tmp_isr],255
 jmp call_isr_handler
