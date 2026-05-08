#![no_std]
#![no_main]

use user::{exec, exit, fork, print, println, read, wait};

const MAXPATH_LEN: usize = 64;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut path = [0u8; MAXPATH_LEN + 1];

    loop {
        print!("# ");

        let n = read(0, &mut path);
        if n <= 0 {
            println!("[sh] read failed");
            exit(1);
        }

        let n = n as usize;
        if n == path.len() && path[n - 1] != b'\n' {
            println!("[sh] path too long");
            discard_line();
            continue;
        }

        let path = trim_input(&path[..n]);
        if path.is_empty() {
            continue;
        }

        let pid = fork();
        if pid < 0 {
            println!("[sh] fork failed");
            continue;
        }

        if pid == 0 {
            if exec(path) < 0 {
                println!("[sh] exec failed");
                exit(1);
            }
        } else {
            let mut status = 0;
            if wait(&mut status) < 0 {
                println!("[sh] wait failed");
                exit(1);
            }
        }
    }
}

fn trim_input(mut s: &[u8]) -> &[u8] {
    while matches!(s.first(), Some(b' ' | b'\t')) {
        s = &s[1..];
    }

    while matches!(s.last(), Some(b'\n' | b'\r' | b' ' | b'\t')) {
        s = &s[..s.len() - 1];
    }

    s
}

fn discard_line() {
    let mut buf = [0u8; 16];

    loop {
        let n = read(0, &mut buf);
        if n <= 0 {
            return;
        }

        if buf[..n as usize].contains(&b'\n') {
            return;
        }
    }
}
