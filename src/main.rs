#![no_std]
#![no_main]

mod console;
mod cpu;
mod kalloc;
mod memlayout;
mod plic;
mod spinlock;
mod timer;
mod trap;
mod uart;
mod vm;

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
    trap::init();
    timer::init();
    plic::init();
    kalloc::init();

    println!("hartid = {}, dtb = {:#x}", hartid, dtb);
    println!("trap initialized");
    println!("timer initialized");

    let pt = vm::kvmmake();
    vm::kvminithart(pt);
    println!("paging on");

    cpu::intr_on();

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
