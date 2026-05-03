#![no_std]

use core::arch::asm;
use core::panic::PanicInfo;

// NOTE: keep in sync with /src/syscall.rs

pub const SYS_EXIT: usize = 93;
pub const SYS_PUTC: usize = 1024; // テスト用

#[inline]
pub unsafe fn syscall6(
    num: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) -> i64 {
    let ret: i64;
    unsafe {
        asm!(
            "ecall",
            in("a7") num,
            inlateout("a0") a0 => ret,
            in("a1") a1,
            in("a2") a2,
            in("a3") a3,
            in("a4") a4,
            in("a5") a5,
            options(nostack),
        );
    }
    ret
}

#[inline]
pub fn exit(code: i32) -> ! {
    unsafe {
        syscall6(SYS_EXIT, code as usize, 0, 0, 0, 0, 0);
    }
    loop {}
}

#[inline]
pub fn putc(c: u8) {
    unsafe {
        syscall6(SYS_PUTC, c as usize, 0, 0, 0, 0, 0);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(255);
}
