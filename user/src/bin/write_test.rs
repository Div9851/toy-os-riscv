#![no_std]
#![no_main]

use user::{O_CREATE, O_RDONLY, O_RDWR, O_WRONLY, close, exit, open, println, read, write};

const PATH: &[u8] = b"/README.md";
const MESSAGE: &[u8] = b"write_test ok\n";
const CREATED: &[u8] = b"/created.txt";
const CREATED_MESSAGE: &[u8] = b"created by write_test\n";

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let fd = open(PATH, O_RDWR) as i32;
    if fd < 0 {
        println!("open for write failed");
        exit(1);
    }

    let n = write(fd, MESSAGE);
    if n != MESSAGE.len() as isize {
        println!("write failed");
        close(fd);
        exit(1);
    }
    close(fd);

    let fd = open(PATH, O_RDONLY) as i32;
    if fd < 0 {
        println!("open for read failed");
        exit(1);
    }

    let mut buf = [0u8; MESSAGE.len()];
    let n = read(fd, &mut buf);
    close(fd);

    if n != MESSAGE.len() as isize || buf != *MESSAGE {
        println!("readback mismatch");
        exit(1);
    }

    let fd = open(CREATED, O_CREATE | O_WRONLY) as i32;
    if fd < 0 {
        println!("create failed");
        exit(1);
    }

    let n = write(fd, CREATED_MESSAGE);
    if n != CREATED_MESSAGE.len() as isize {
        println!("created write failed");
        close(fd);
        exit(1);
    }
    close(fd);

    let fd = open(CREATED, O_RDONLY) as i32;
    if fd < 0 {
        println!("created open failed");
        exit(1);
    }

    let mut buf = [0u8; CREATED_MESSAGE.len()];
    let n = read(fd, &mut buf);
    close(fd);

    if n != CREATED_MESSAGE.len() as isize || buf != *CREATED_MESSAGE {
        println!("created readback mismatch");
        exit(1);
    }

    println!("write_test ok");
    exit(0);
}
