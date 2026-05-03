#![no_std]
#![no_main]

use user::{exit, putc};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    putc(b'B');
    exit(0);
}
