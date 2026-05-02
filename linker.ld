OUTPUT_ARCH("riscv")
ENTRY(_start)

SECTIONS {
    . = 0x80200000;

    .text : {
        KEEP(*(.text.entry))
        . = ALIGN(4096);
        __trampoline_start = .;
        KEEP(*(.text.trampoline))
        . = ALIGN(4096);
        __trampoline_end = .;
        *(.text .text.*)
        . = ALIGN(4096);
        __etext = .;
    }

    .rodata : ALIGN(4096) {
        *(.rodata .rodata.*)
        . = ALIGN(4096);
        __erodata = .;
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
    __kernel_end = .; /* kalloc::init relies on this alignment */
}