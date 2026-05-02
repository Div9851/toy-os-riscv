use core::arch::asm;
use core::ptr::addr_of_mut;

pub struct Cpu {
    pub noff: usize,  // push_off の入れ子の深さ
    pub intena: bool, // 最外 push_off 時の SIE の状態
    pub proc: *mut crate::proc::Process,
}

// シングルコア前提で 1 個。SMP に行くときは hartid で配列化する。
static mut CPU: Cpu = Cpu {
    noff: 0,
    intena: false,
    proc: core::ptr::null_mut(),
};

#[inline]
pub fn mycpu() -> &'static mut Cpu {
    unsafe { &mut *addr_of_mut!(CPU) }
}

fn intr_get() -> bool {
    let s: usize;
    unsafe {
        asm!("csrr {0}, sstatus", out(reg) s);
    }
    (s >> 1) & 1 == 1
}

pub fn intr_off() {
    unsafe {
        asm!("csrc sstatus, {0}", in(reg) 1usize<<1);
    }
}

pub fn intr_on() {
    unsafe {
        asm!("csrs sstatus, {0}", in(reg) 1usize<<1);
    }
}

pub fn push_off() {
    let old = intr_get();
    intr_off();
    let cpu = mycpu();
    if cpu.noff == 0 {
        cpu.intena = old;
    }
    cpu.noff += 1;
}

pub fn pop_off() {
    // この時点で SIE は OFF のはず。
    assert!(!intr_get(), "pop_off: interrupts enabled");
    let cpu = mycpu();
    assert!(cpu.noff >= 1, "pop_off: not pushed");
    cpu.noff -= 1;
    if cpu.noff == 0 && cpu.intena {
        intr_on();
    }
}

pub unsafe fn r_satp() -> u64 {
    let x: u64;
    unsafe {
        asm!("csrr {0}, satp", out(reg) x);
    }
    x
}

pub unsafe fn w_satp(x: u64) {
    unsafe {
        asm!("csrw satp, {0}", in(reg) x);
    }
}

pub unsafe fn sfence_vma() {
    // sfence.vma zero, zero  → 全 VA / 全 ASID をフラッシュ
    unsafe {
        asm!("sfence.vma zero, zero");
    }
}

pub unsafe fn r_sepc() -> usize {
    let x: usize;
    asm!("csrr {}, sepc", out(reg) x);
    x
}

pub unsafe fn w_sepc(x: usize) {
    asm!("csrw sepc, {}", in(reg) x);
}

pub unsafe fn r_scause() -> usize {
    let x: usize;
    asm!("csrr {}, scause", out(reg) x);
    x
}

pub unsafe fn r_sstatus() -> usize {
    let x: usize;
    asm!("csrr {}, sstatus", out(reg) x);
    x
}

pub unsafe fn w_sstatus(x: usize) {
    asm!("csrw sstatus, {}", in(reg) x);
}

pub unsafe fn w_stvec(x: usize) {
    asm!("csrw stvec, {}", in(reg) x);
}

pub unsafe fn r_tp() -> usize {
    let x: usize;
    asm!("mv {}, tp", out(reg) x);
    x
}

pub const SSTATUS_SPP: usize = 1 << 8;
pub const SSTATUS_SPIE: usize = 1 << 5;
