#![no_std]
#![no_main]

use alloc::vec::Vec;
use user::{
    O_APPEND, O_CREATE, O_RDONLY, O_TRUNC, O_WRONLY, chdir, close, exec, exit, fork, open, print,
    println, read, wait,
};

extern crate alloc;

const MAXLEN: usize = 64;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut line = [0u8; MAXLEN + 1];

    loop {
        print!("# ");

        let n = read(0, &mut line);
        if n == 0 {
            exit(0);
        }
        if n < 0 {
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

        let tokens: Vec<&[u8]> = line
            .split(|b| matches!(b, b' ' | b'\t'))
            .filter(|arg| !arg.is_empty())
            .collect();

        let cmd = match parse_command(&tokens) {
            Some(cmd) => cmd,
            None => {
                println!("[sh] syntax error");
                continue;
            }
        };

        if cmd.argv.is_empty() {
            continue;
        }

        if cmd.argv[0] == b"cd" {
            if cmd.argv.len() != 2 {
                println!("[sh] usage: cd DIR");
                continue;
            }
            if chdir(cmd.argv[1]) < 0 {
                println!("[sh] cd failed");
            }
            continue;
        }

        if cmd.argv[0] == b"exit" {
            exit(0);
        }

        let path = resolve_command(&cmd.argv[0]);

        let pid = fork();
        if pid < 0 {
            println!("[sh] fork failed");
            continue;
        }

        if pid == 0 {
            if !apply_redirects(&cmd) {
                println!("[sh] redirect failed");
                exit(1);
            }
            if exec(&path, &cmd.argv) < 0 {
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

struct Command<'a> {
    argv: Vec<&'a [u8]>,
    input: Option<&'a [u8]>,
    output: Option<OutputRedirect<'a>>,
}

struct OutputRedirect<'a> {
    path: &'a [u8],
    append: bool,
}

fn parse_command<'a>(tokens: &[&'a [u8]]) -> Option<Command<'a>> {
    let mut argv = Vec::new();
    let mut input = None;
    let mut output = None;
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i] {
            b"<" => {
                i += 1;
                if i >= tokens.len() || input.is_some() {
                    return None;
                }
                input = Some(tokens[i]);
            }
            b">" | b">>" => {
                let append = tokens[i] == b">>";
                i += 1;
                if i >= tokens.len() || output.is_some() {
                    return None;
                }
                output = Some(OutputRedirect {
                    path: tokens[i],
                    append,
                });
            }
            arg => argv.push(arg),
        }
        i += 1;
    }

    Some(Command {
        argv,
        input,
        output,
    })
}

fn apply_redirects(cmd: &Command<'_>) -> bool {
    if let Some(path) = cmd.input {
        close(0);
        if open(path, O_RDONLY) != 0 {
            return false;
        }
    }

    if let Some(out) = &cmd.output {
        close(1);
        let flags = if out.append {
            O_CREATE | O_WRONLY | O_APPEND
        } else {
            O_CREATE | O_WRONLY | O_TRUNC
        };
        if open(out.path, flags) != 1 {
            return false;
        }
    }

    true
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
