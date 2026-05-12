#![no_std]
#![no_main]

use alloc::vec::Vec;
use user::{
    O_APPEND, O_CREATE, O_RDONLY, O_TRUNC, O_WRONLY, chdir, close, dup, exec, exit, fork, open,
    pipe, print, println, read, wait,
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

        let job = match parse_job(&tokens) {
            Some(job) => job,
            None => {
                println!("[sh] syntax error");
                continue;
            }
        };

        match job {
            Job::Single(cmd) => run_single(&cmd),
            Job::Pipeline(cmds) => run_pipeline(&cmds),
        }
    }
}

enum Job<'a> {
    Single(Command<'a>),
    Pipeline(Vec<Command<'a>>),
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

fn parse_job<'a>(tokens: &[&'a [u8]]) -> Option<Job<'a>> {
    let mut cmds = Vec::new();
    let mut start = 0;

    for (i, token) in tokens.iter().enumerate() {
        if *token == b"|" {
            if i == start {
                return None;
            }
            cmds.push(parse_command(&tokens[start..i])?);
            start = i + 1;
        }
    }

    if start >= tokens.len() {
        return None;
    }
    cmds.push(parse_command(&tokens[start..])?);

    for cmd in &cmds {
        if cmd.argv.is_empty() {
            return None;
        }
    }

    if cmds.len() == 1 {
        Some(Job::Single(cmds.remove(0)))
    } else {
        for (i, cmd) in cmds.iter().enumerate() {
            if i != 0 && cmd.input.is_some() {
                return None;
            }
            if i + 1 != cmds.len() && cmd.output.is_some() {
                return None;
            }
        }

        Some(Job::Pipeline(cmds))
    }
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
            b"|" => {
                return None;
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

fn run_single(cmd: &Command<'_>) {
    if cmd.argv.is_empty() {
        return;
    }

    if cmd.argv[0] == b"cd" {
        if cmd.argv.len() != 2 {
            println!("[sh] usage: cd DIR");
            return;
        }
        if chdir(cmd.argv[1]) < 0 {
            println!("[sh] cd failed");
        }
        return;
    }

    if cmd.argv[0] == b"exit" {
        exit(0);
    }

    let pid = fork();
    if pid < 0 {
        println!("[sh] fork failed");
        return;
    }

    if pid == 0 {
        exec_command(cmd);
    }

    let mut status = 0;
    if wait(&mut status) < 0 {
        println!("[sh] wait failed");
        exit(1);
    }
}

fn run_pipeline(cmds: &[Command<'_>]) {
    let mut pipes = Vec::new();

    for _ in 0..cmds.len() - 1 {
        let mut fds = [0i32; 2];
        if pipe(&mut fds) < 0 {
            close_pipes(&pipes);
            println!("[sh] pipe failed");
            return;
        }
        pipes.push(fds);
    }

    let mut started = 0;

    for i in 0..cmds.len() {
        let pid = fork();
        if pid < 0 {
            close_pipes(&pipes);
            wait_children(started);
            println!("[sh] fork failed");
            return;
        }

        if pid == 0 {
            setup_pipeline_fds(i, &pipes);
            exec_command(&cmds[i]);
        }

        started += 1;
    }

    close_pipes(&pipes);
    wait_children(started);
}

fn setup_pipeline_fds(i: usize, pipes: &[[i32; 2]]) {
    if i > 0 {
        close(0);
        if dup(pipes[i - 1][0]) != 0 {
            println!("[sh] pipe redirect failed");
            exit(1);
        }
    }

    if i < pipes.len() {
        close(1);
        if dup(pipes[i][1]) != 1 {
            println!("[sh] pipe redirect failed");
            exit(1);
        }
    }

    close_pipes(pipes);
}

fn close_pipes(pipes: &[[i32; 2]]) {
    for fds in pipes {
        close(fds[0]);
        close(fds[1]);
    }
}

fn wait_children(n: usize) {
    let mut status = 0;

    for _ in 0..n {
        if wait(&mut status) < 0 {
            println!("[sh] wait failed");
            exit(1);
        }
    }
}

fn exec_command(cmd: &Command<'_>) -> ! {
    let path = resolve_command(&cmd.argv[0]);

    if !apply_redirects(cmd) {
        println!("[sh] redirect failed");
        exit(1);
    }

    if exec(&path, &cmd.argv) < 0 {
        println!("[sh] exec failed");
        exit(1);
    }

    exit(1);
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
