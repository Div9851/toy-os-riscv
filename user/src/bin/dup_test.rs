#![no_std]
#![no_main]

use user::{
    O_CREATE, O_RDONLY, O_TRUNC, O_WRONLY, close, dup, exit, open, println, read, write_all,
};

const PATH: &[u8] = b"/dup_test.txt";
const EXPECTED: &[u8] = b"onetwothree";

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let fd = open(PATH, O_CREATE | O_WRONLY | O_TRUNC) as i32;
    if fd < 0 {
        println!("open failed");
        exit(1);
    }

    if write_all(fd, b"one") < 0 {
        println!("first write failed");
        close(fd);
        exit(1);
    }

    let fd2 = dup(fd) as i32;
    if fd2 < 0 {
        println!("dup failed");
        close(fd);
        exit(1);
    }

    if write_all(fd, b"two") < 0 || write_all(fd2, b"three") < 0 {
        println!("shared write failed");
        close(fd);
        close(fd2);
        exit(1);
    }

    close(fd);
    close(fd2);

    let fd = open(PATH, O_RDONLY) as i32;
    if fd < 0 {
        println!("read open failed");
        exit(1);
    }

    let mut buf = [0u8; EXPECTED.len()];
    let n = read(fd, &mut buf);
    close(fd);

    if n != EXPECTED.len() as isize || buf != *EXPECTED {
        println!("dup_test mismatch");
        exit(1);
    }

    println!("dup_test ok");
    exit(0);
}
