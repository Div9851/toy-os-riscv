#![no_std]
#![no_main]

use user::{exit, fork, write};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let pid = fork();
    if pid == 0 {
        write(1, b"child\n");
        exit(0);
    } else if pid > 0 {
        write(1, b"parent\n");
        exit(0);
    } else {
        write(1, b"fork failed\n");
        exit(1);
    }
}
