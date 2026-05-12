#![no_std]
#![no_main]

mod console;
mod cpu;
mod file;
mod fs;
mod kalloc;
mod loader;
mod memlayout;
mod plic;
mod proc;
mod spinlock;
mod syscall;
mod timer;
mod trap;
mod uart;
mod vm;

use core::arch::global_asm;
use core::panic::PanicInfo;

global_asm!(include_str!("asm/entry.S"));
global_asm!(include_str!("asm/trampoline.S"));
global_asm!(include_str!("asm/swtch.S"));

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

    let kpt = vm::kvmmake();
    vm::kvminithart(kpt);
    println!("paging on");

    fs::selftest();
    println!("fs selftest ok");
    fs::init();

    cpu::intr_on();

    proc::userinit();
    proc::scheduler();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    emergency_println!("\n!!! KERNEL PANIC: {}", info);
    loop {
        core::hint::spin_loop();
    }
}
