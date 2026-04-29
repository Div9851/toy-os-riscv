OUTPUT_ARCH("riscv")
ENTRY(_start)

SECTIONS {
    . = 0x80200000;

    .text : {
        KEEP(*(.text.entry))
        *(.text .text.*)
    }

    .rodata : ALIGN(8) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(8) {
        *(.data .data.*)
    }

    .bss : ALIGN(8) {
        __bss_start = .;
        *(.bss .bss.* COMMON)
        . = ALIGN(8);
        __bss_end = .;
    }

    . = ALIGN(16);
    . += 64K;
    __stack_top = .;

    . = ALIGN(4096);
    __kernel_end = .;
}