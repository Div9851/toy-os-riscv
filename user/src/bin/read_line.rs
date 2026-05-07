#![no_std]
#![no_main]

use user::{exit, println, read, write_all};

#[unsafe(no_mangle)]
pub extern "C" fn _start(_argc: usize, _argv: *const *const u8) -> ! {
    println!("type a line:");
    let mut buf = [0u8; 64];
    let n = read(0, &mut buf);
    if n > 0 {
        write_all(1, b"read: ");
        write_all(1, &buf[..n as usize]);
        exit(0);
    } else {
        println!("read failed");
        exit(1);
    }
}
