use core::arch::{asm, naked_asm};

use crate::{
    cpu,
    memlayout::{PGSIZE, TRAPFRAME, trampoline_userret_va, trampoline_uservec_va},
    println, proc, syscall, vm,
};

pub fn init() {
    let addr: usize = trap_entry as *const () as usize;
    unsafe {
        asm!("csrw stvec, {0}", in(reg) addr);
    }

    let read_back: usize;
    unsafe {
        asm!("csrr {0}, stvec", out(reg) read_back);
    }

    println!("stvec = {:#x}", read_back);
}

#[unsafe(naked)]
extern "C" fn trap_entry() -> ! {
    naked_asm!(
        ".align 2", // stvec 用 4-byte align
        "addi sp, sp, -256",
        "sd ra, 0(sp)",
        "sd sp, 8(sp)",
        "sd gp, 16(sp)",
        "sd tp, 24(sp)",
        "sd t0, 32(sp)",
        "sd t1, 40(sp)",
        "sd t2, 48(sp)",
        "sd s0, 56(sp)",
        "sd s1, 64(sp)",
        "sd a0, 72(sp)",
        "sd a1, 80(sp)",
        "sd a2, 88(sp)",
        "sd a3, 96(sp)",
        "sd a4, 104(sp)",
        "sd a5, 112(sp)",
        "sd a6, 120(sp)",
        "sd a7, 128(sp)",
        "sd s2, 136(sp)",
        "sd s3, 144(sp)",
        "sd s4, 152(sp)",
        "sd s5, 160(sp)",
        "sd s6, 168(sp)",
        "sd s7, 176(sp)",
        "sd s8, 184(sp)",
        "sd s9, 192(sp)",
        "sd s10, 200(sp)",
        "sd s11, 208(sp)",
        "sd t3, 216(sp)",
        "sd t4, 224(sp)",
        "sd t5, 232(sp)",
        "sd t6, 240(sp)",
        "call kerneltrap",
        "ld ra, 0(sp)",
        // ld sp は不要
        "ld gp, 16(sp)",
        "ld tp, 24(sp)",
        "ld t0, 32(sp)",
        "ld t1, 40(sp)",
        "ld t2, 48(sp)",
        "ld s0, 56(sp)",
        "ld s1, 64(sp)",
        "ld a0, 72(sp)",
        "ld a1, 80(sp)",
        "ld a2, 88(sp)",
        "ld a3, 96(sp)",
        "ld a4, 104(sp)",
        "ld a5, 112(sp)",
        "ld a6, 120(sp)",
        "ld a7, 128(sp)",
        "ld s2, 136(sp)",
        "ld s3, 144(sp)",
        "ld s4, 152(sp)",
        "ld s5, 160(sp)",
        "ld s6, 168(sp)",
        "ld s7, 176(sp)",
        "ld s8, 184(sp)",
        "ld s9, 192(sp)",
        "ld s10, 200(sp)",
        "ld s11, 208(sp)",
        "ld t3, 216(sp)",
        "ld t4, 224(sp)",
        "ld t5, 232(sp)",
        "ld t6, 240(sp)",
        "addi sp, sp, 256",
        "sret",
    )
}

#[unsafe(no_mangle)]
extern "C" fn kerneltrap() {
    let scause: usize;
    unsafe {
        asm!("csrr {0}, scause", out(reg) scause);
    }

    let is_interrupt = (scause >> 63) & 1 == 1;
    let code = scause & 0xff;

    if is_interrupt {
        match code {
            5 => crate::timer::handle(),
            9 => crate::plic::handle_external(),
            _ => panic!("kerneltrap: unexpected interrupt code={}", code),
        }
    } else {
        let sepc: usize;
        let stval: usize;
        let sstatus: usize;
        unsafe {
            asm!("csrr {0}, sepc",    out(reg) sepc);
            asm!("csrr {0}, stval",   out(reg) stval);
            asm!("csrr {0}, sstatus", out(reg) sstatus);
        }
        panic!(
            "kerneltrap: scause={:#x} sepc={:#x} stval={:#x} sstatus={:#x}",
            scause, sepc, stval, sstatus
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn usertrap() -> ! {
    let sstatus = unsafe { cpu::r_sstatus() };
    assert_eq!(
        sstatus & cpu::SSTATUS_SPP,
        0,
        "usertrap: not from user mode"
    );

    let kernelvec = trap_entry as *const () as usize;
    unsafe {
        cpu::w_stvec(kernelvec);
    }

    let p = unsafe { &mut *proc::myproc() };

    unsafe {
        (*p.trapframe).epc = cpu::r_sepc() as u64;
    }

    let scause = unsafe { cpu::r_scause() };

    match scause {
        8 => syscall::syscall(),
        _ => panic!("usertrap: unhandled scause = {:#x}", scause),
    }

    usertrapret()
}

pub fn usertrapret() -> ! {
    let p = unsafe { &mut *proc::myproc() };

    cpu::intr_off();

    unsafe {
        cpu::w_stvec(trampoline_uservec_va());
    }

    unsafe {
        (*p.trapframe).kernel_satp = cpu::r_satp() as u64;
        (*p.trapframe).kernel_sp = (p.kstack + PGSIZE) as u64;
        (*p.trapframe).kernel_trap = usertrap as *const () as u64;
        (*p.trapframe).kernel_hartid = cpu::r_tp() as u64;
    }

    unsafe {
        let mut s = cpu::r_sstatus();
        s &= !cpu::SSTATUS_SPP; // SPP = 0 → U-mode へ
        s |= cpu::SSTATUS_SPIE; // SPIE = 1 → sret 後 SIE = 1
        cpu::w_sstatus(s);

        cpu::w_sepc((*p.trapframe).epc as usize);
    }

    let satp = vm::make_satp(p.pagetable);

    let userret_va = trampoline_userret_va();
    let userret_fn: extern "C" fn(usize, usize) -> ! = unsafe { core::mem::transmute(userret_va) };
    userret_fn(TRAPFRAME, satp as usize);
}
