#![no_std]
#![no_main]

use user::{close, exit, fork, pipe, println, read, wait, write_all};

const MSG: &[u8] = b"pipe works";

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut fds = [0i32; 2];
    if pipe(&mut fds) < 0 {
        println!("pipe_test: pipe failed");
        exit(1);
    }

    let pid = fork();
    if pid < 0 {
        println!("pipe_test: fork failed");
        exit(1);
    }

    if pid == 0 {
        close(fds[0]);
        if write_all(fds[1], MSG) < 0 {
            println!("pipe_test: write failed");
            exit(1);
        }
        close(fds[1]);
        exit(0);
    }

    close(fds[1]);

    let mut buf = [0u8; MSG.len()];
    let n = read(fds[0], &mut buf);
    close(fds[0]);

    let mut status = 0;
    wait(&mut status);

    if n != MSG.len() as isize || buf.as_slice() != MSG || status != 0 {
        println!("pipe_test: mismatch");
        exit(1);
    }

    println!("pipe_test ok");
    exit(0);
}
