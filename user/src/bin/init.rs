#![no_std]
#![no_main]

use user::{exec, exit, fork, println, wait};

const SHELL: &[u8] = b"/bin/sh";

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {
        let shell_pid = fork();
        if shell_pid < 0 {
            println!("[init] failed to fork shell");
            exit(1);
        }

        if shell_pid == 0 {
            exec(SHELL, &[SHELL]);
            println!("[init] failed to exec shell");
            exit(1);
        }

        loop {
            let mut status = 0;
            let pid = wait(&mut status);
            if pid < 0 {
                println!("[init] wait failed");
                break;
            }
            if pid == shell_pid {
                break;
            }
        }
    }
}
