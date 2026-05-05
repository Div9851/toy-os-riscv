#![no_std]
#![no_main]

use user::{exit, fork, getpid, println, wait, write};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let pid = fork();

    if pid == 0 {
        let child_pid = getpid();
        println!("[child] pid={}", child_pid);
        exit(42);
    } else if pid > 0 {
        let parent_pid = getpid();
        println!("[parent] pid={}", parent_pid);

        let mut status = 0;
        let waited = wait(&mut status);

        println!("[parent] child pid={}", waited);
        println!("[parent] exit status={}", status);

        exit(0);
    } else {
        write(1, b"fork failed\n");
        exit(1);
    }
}
