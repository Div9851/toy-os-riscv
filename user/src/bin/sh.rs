#![no_std]
#![no_main]

use alloc::vec::Vec;
use user::{chdir, exec, exit, fork, print, println, read, wait};

extern crate alloc;

const MAXLEN: usize = 64;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut line = [0u8; MAXLEN + 1];

    loop {
        print!("# ");

        let n = read(0, &mut line);
        if n <= 0 {
            println!("[sh] read failed");
            exit(1);
        }

        let n = n as usize;
        if n == line.len() && line[n - 1] != b'\n' {
            println!("[sh] path too long");
            discard_line();
            continue;
        }

        let line = trim_input(&line[..n]);
        if line.is_empty() {
            continue;
        }

        let argv: Vec<&[u8]> = line
            .split(|b| matches!(b, b' ' | b'\t'))
            .filter(|arg| !arg.is_empty())
            .collect();

        if argv.is_empty() {
            continue;
        }

        if argv[0] == b"cd" {
            if argv.len() != 2 {
                println!("[sh] usage: cd DIR");
                continue;
            }
            if chdir(argv[1]) < 0 {
                println!("[sh] cd failed");
            }
            continue;
        }

        if argv[0] == b"exit" {
            exit(0);
        }

        let cmd = resolve_command(&argv[0]);

        let pid = fork();
        if pid < 0 {
            println!("[sh] fork failed");
            continue;
        }

        if pid == 0 {
            if exec(&cmd, &argv) < 0 {
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

fn resolve_command(cmd: &[u8]) -> Vec<u8> {
    if cmd.contains(&b'/') {
        cmd.to_vec()
    } else {
        let mut path = Vec::new();
        path.extend_from_slice(b"/bin/");
        path.extend_from_slice(cmd);
        path
    }
}
