.section .rodata.initcode, "a"
.globl __initcode_payload
__initcode_payload:
    li a0, 'A' # 0x41
    li a7, 1024 # SYS_PUTC
    ecall
    li a0, 0
    li a7, 93 # SYS_EXIT
    ecall
1:  j 1b
