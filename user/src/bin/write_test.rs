#![no_std]
#![no_main]

use user::{
    O_APPEND, O_CREATE, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY, close, exit, open, println, read,
    write, write_all,
};

const PATH: &[u8] = b"/README.md";
const MESSAGE: &[u8] = b"write_test ok\n";
const CREATED: &[u8] = b"/created.txt";
const CREATED_MESSAGE: &[u8] = b"created by write_test\n";
const TRUNCATED: &[u8] = b"/truncated.txt";
const TRUNCATED_MESSAGE: &[u8] = b"truncated\n";
const APPENDED: &[u8] = b"/appended.txt";
const APPEND_EXPECTED: &[u8] = b"first\nsecond\n";
const CHUNK: &[u8; 128] =
    b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

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

    let fd = open(TRUNCATED, O_CREATE | O_WRONLY) as i32;
    if fd < 0 {
        println!("truncate create failed");
        exit(1);
    }

    for _ in 0..110 {
        if write_all(fd, CHUNK) < 0 {
            println!("truncate setup write failed");
            close(fd);
            exit(1);
        }
    }
    close(fd);

    let fd = open(TRUNCATED, O_WRONLY | O_TRUNC) as i32;
    if fd < 0 {
        println!("truncate open failed");
        exit(1);
    }

    let n = write(fd, TRUNCATED_MESSAGE);
    if n != TRUNCATED_MESSAGE.len() as isize {
        println!("truncate write failed");
        close(fd);
        exit(1);
    }
    close(fd);

    let fd = open(TRUNCATED, O_RDONLY) as i32;
    if fd < 0 {
        println!("truncate read open failed");
        exit(1);
    }

    let mut buf = [0u8; TRUNCATED_MESSAGE.len() + 1];
    let n = read(fd, &mut buf);
    close(fd);

    if n != TRUNCATED_MESSAGE.len() as isize || buf[..TRUNCATED_MESSAGE.len()] != *TRUNCATED_MESSAGE
    {
        println!("truncate readback mismatch");
        exit(1);
    }

    let fd = open(APPENDED, O_CREATE | O_WRONLY | O_TRUNC) as i32;
    if fd < 0 {
        println!("append create failed");
        exit(1);
    }
    if write_all(fd, b"first\n") < 0 {
        println!("append first write failed");
        close(fd);
        exit(1);
    }
    close(fd);

    let fd = open(APPENDED, O_WRONLY | O_APPEND) as i32;
    if fd < 0 {
        println!("append open failed");
        exit(1);
    }
    if write_all(fd, b"second\n") < 0 {
        println!("append second write failed");
        close(fd);
        exit(1);
    }
    close(fd);

    let fd = open(APPENDED, O_RDONLY) as i32;
    if fd < 0 {
        println!("append read open failed");
        exit(1);
    }
    let mut buf = [0u8; APPEND_EXPECTED.len()];
    let n = read(fd, &mut buf);
    close(fd);

    if n != APPEND_EXPECTED.len() as isize || buf != *APPEND_EXPECTED {
        println!("append readback mismatch");
        exit(1);
    }

    println!("write_test ok");
    exit(0);
}
