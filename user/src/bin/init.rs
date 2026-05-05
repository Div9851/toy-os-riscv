#![no_std]
#![no_main]

use user::{exit, write};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    write(1, b"start\n");

    for _ in 0..5 {
        let mut n = 0usize;
        while n < 200_000_000 {
            unsafe {
                core::arch::asm!("", options(nomem, nostack, preserves_flags));
            }
            n += 1;
        }

        write(1, b".\n");
    }

    write(1, b"done\n");
    exit(0);
}
