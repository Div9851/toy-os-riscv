#![no_std]
#![no_main]

mod console;
mod memlayout;
mod uart;

use core::arch::global_asm;
use core::panic::PanicInfo;

global_asm!(
    r#"
.section .text.entry
.global _start
_start:
    la sp, __stack_top

    la t0, __bss_start
    la t1, __bss_end
1:
    bgeu t0, t1, 2f
    sd zero, 0(t0)
    addi t0, t0, 8
    j 1b
2:
    tail kmain
"#
);

#[unsafe(no_mangle)]
extern "C" fn kmain(hartid: usize, dtb: usize) -> ! {
    console::init();
    println!("Hello, world!");
    println!("hartid = {}, dtb = {:#x}", hartid, dtb);
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n!!! KERNEL PANIC: {}", info);
    loop {
        core::hint::spin_loop();
    }
}
