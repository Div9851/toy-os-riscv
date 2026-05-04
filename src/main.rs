#![no_std]
#![no_main]

mod console;
mod cpu;
mod exec;
mod kalloc;
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

    cpu::intr_on();

    let mut p = proc::Process::new();
    // unsafe {
    //     vm::uvmfirst(&mut *p.pagetable, memlayout::initcode());
    // }
    // p.sz = memlayout::PGSIZE;
    let (entry, sp, sz) =
        exec::exec(unsafe { &mut *p.pagetable }, exec::INIT_ELF).expect("exec init");
    p.sz = sz;
    unsafe {
        (*p.trapframe).epc = entry as u64;
        (*p.trapframe).sp = sp as u64;
    }

    (*cpu::mycpu()).proc = &mut p;

    trap::usertrapret();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    emergency_println!("\n!!! KERNEL PANIC: {}", info);
    loop {
        core::hint::spin_loop();
    }
}
