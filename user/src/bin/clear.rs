#![no_std]
#![no_main]

use user::{exit, write_all};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    write_all(1, b"\x1b[2J\x1b[H");
    exit(0);
}
