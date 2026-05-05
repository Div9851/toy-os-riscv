#![no_std]
#![no_main]

use user::{exit, fork, getpid, wait, write};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let before = getpid();
    let pid = fork();

    if pid == 0 {
        let child_pid = getpid();
        if child_pid > 0 && child_pid != before {
            write(1, b"child getpid ok\n");
            exit(42);
        } else {
            write(1, b"child getpid failed\n");
            exit(1);
        }
    } else if pid > 0 {
        let parent_pid = getpid();
        let mut status = 0;
        let waited = wait(&mut status);

        if parent_pid == before && waited == pid && status == 42 {
            write(1, b"parent getpid ok\n");
            exit(0);
        } else {
            write(1, b"parent getpid failed\n");
            exit(1);
        }
    } else {
        write(1, b"fork failed\n");
        exit(1);
    }
}
