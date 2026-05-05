use core::arch::asm;

use crate::proc;

// QEMU virt の mtime は 10 MHz。1秒 = 10_000_000 tick。
const INTERVAL: u64 = 1_000_000; // 100ms

pub fn init() {
    schedule_next();

    unsafe {
        // sie.SITE = 1 (bit 5) - supervisor timer interrupt enable
        asm!("csrs sie, {0}", in(reg) 1usize << 5);
    }
}

pub fn handle() {
    schedule_next();

    if !proc::myproc().is_null() {
        proc::yield_cpu();
    }
}

fn schedule_next() {
    let next = rdtime() + INTERVAL;
    sbi_set_timer(next);
}

fn rdtime() -> u64 {
    let t: u64;
    unsafe {
        asm!("rdtime {0}", out(reg) t);
    }
    t
}

fn sbi_set_timer(stime_value: u64) {
    const EID: usize = 0x5449_4D45; // "TIME"
    const FID: usize = 0;
    unsafe {
        asm!("ecall", in("a7") EID, in("a6") FID, in("a0") stime_value, lateout("a0") _, lateout("a1") _,);
    }
}
