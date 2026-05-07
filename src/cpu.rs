use core::arch::asm;

pub struct Cpu {
    pub noff: usize,  // Nested push_off depth.
    pub intena: bool, // SIE state before the outermost push_off.
    pub proc: *mut crate::proc::Process,
    pub context: crate::proc::Context,
}

// Single-hart for now. SMP will turn this into a per-hart array indexed by hartid.
static mut CPU: Cpu = Cpu {
    noff: 0,
    intena: false,
    proc: core::ptr::null_mut(),
    context: crate::proc::Context::zero(),
};

#[inline]
pub fn mycpu() -> &'static mut Cpu {
    unsafe { &mut *mycpu_ptr() }
}

pub fn mycpu_ptr() -> *mut Cpu {
    core::ptr::addr_of_mut!(CPU)
}

#[inline]
pub fn cpuid() -> usize {
    0
}

pub fn intr_get() -> bool {
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

/// Disable interrupts and remember the previous interrupt state at the outer
/// nesting level.
///
/// Spinlocks use this to avoid interrupt handlers re-entering code protected by
/// the same lock on this hart. `noff` counts nested critical sections, and
/// `intena` records whether SIE should be restored by the final `pop_off`.
pub fn push_off() {
    let old = intr_get();
    intr_off();
    let cpu = mycpu();
    if cpu.noff == 0 {
        cpu.intena = old;
    }
    cpu.noff += 1;
}

/// Leave one `push_off` critical section.
///
/// Interrupts must still be disabled when this is called. The final pop restores
/// SIE only if it was enabled before the outermost `push_off`.
pub fn pop_off() {
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
    // Flush all virtual addresses for all ASIDs.
    unsafe {
        asm!("sfence.vma zero, zero");
    }
}

pub unsafe fn r_sepc() -> usize {
    let x: usize;
    unsafe {
        asm!("csrr {}, sepc", out(reg) x);
    }
    x
}

pub unsafe fn w_sepc(x: usize) {
    unsafe {
        asm!("csrw sepc, {}", in(reg) x);
    }
}

pub unsafe fn r_scause() -> usize {
    let x: usize;
    unsafe {
        asm!("csrr {}, scause", out(reg) x);
    }
    x
}

pub unsafe fn r_stval() -> usize {
    let x: usize;
    unsafe {
        asm!("csrr {}, stval", out(reg) x);
    }
    x
}

pub unsafe fn r_sstatus() -> usize {
    let x: usize;
    unsafe {
        asm!("csrr {}, sstatus", out(reg) x);
    }
    x
}

pub unsafe fn w_sstatus(x: usize) {
    unsafe {
        asm!("csrw sstatus, {}", in(reg) x);
    }
}

pub unsafe fn w_stvec(x: usize) {
    unsafe {
        asm!("csrw stvec, {}", in(reg) x);
    }
}

pub unsafe fn r_tp() -> usize {
    let x: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) x);
    }
    x
}

pub const SSTATUS_SPP: usize = 1 << 8;
pub const SSTATUS_SPIE: usize = 1 << 5;
