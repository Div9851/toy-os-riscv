OUTPUT_ARCH("riscv")
ENTRY(_start)

SECTIONS {
    . = 0;

    .text : {
        *(.text .text.*)
    }

    .rodata : ALIGN(4096) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4096) {
        *(.data .data.*)
    }

    .bss : ALIGN(8) {
        *(.bss .bss.* COMMON)
    }
}