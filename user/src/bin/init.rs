#![no_std]
#![no_main]

use user::{execv_cstr, exit, fork, println, wait, write};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let pid = fork();
    if pid > 0 {
        println!("[parent] wait child");
        let mut status = 0;
        if wait(&mut status) > 0 {
            println!("[parent] child exit status: {}", status);
            exit(0);
        } else {
            println!("[child] wait failed");
            exit(1);
        }
    } else if pid == 0 {
        println!("[child] exec read_line");
        let argv = [b"read_line\0".as_ptr(), core::ptr::null()];
        execv_cstr(b"read_line\0", &argv);
        println!("[child] exec failed");
        exit(1);
    } else {
        write(1, b"fork failed\n");
        exit(1);
    }
}
