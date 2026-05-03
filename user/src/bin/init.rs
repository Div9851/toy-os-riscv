#![no_std]
#![no_main]

use user::{exit, write};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    write(1, b"Hello, world!\n");
    exit(0);
}
