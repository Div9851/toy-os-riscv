#![no_std]
#![no_main]

use user::{Args, O_RDONLY, close, exit, open, println, read, write_all};

#[derive(Clone, Copy)]
struct Count {
    lines: usize,
    words: usize,
    bytes: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: usize, argv: *const *const u8) -> ! {
    let args = Args::new(argc, argv);

    if args.len() == 1 {
        match count_fd(0) {
            Some(count) => print_count(count, None),
            None => exit(1),
        }
        exit(0);
    }

    let mut total = Count {
        lines: 0,
        words: 0,
        bytes: 0,
    };
    let mut ok = true;

    for i in 1..args.len() {
        let path = args.get(i).unwrap();
        let fd = open(path, O_RDONLY) as i32;
        if fd < 0 {
            write_all(1, path);
            println!(": open failed");
            ok = false;
            continue;
        }

        match count_fd(fd) {
            Some(count) => {
                print_count(count, Some(path));
                total.lines += count.lines;
                total.words += count.words;
                total.bytes += count.bytes;
            }
            None => {
                write_all(1, path);
                println!(": read failed");
                ok = false;
            }
        }

        close(fd);
    }

    if args.len() > 2 {
        print_count(total, Some(b"total"));
    }

    exit(if ok { 0 } else { 1 });
}

fn count_fd(fd: i32) -> Option<Count> {
    let mut count = Count {
        lines: 0,
        words: 0,
        bytes: 0,
    };
    let mut in_word = false;
    let mut buf = [0u8; 128];

    loop {
        let n = read(fd, &mut buf);
        if n < 0 {
            return None;
        }
        if n == 0 {
            return Some(count);
        }

        for &b in &buf[..n as usize] {
            count.bytes += 1;

            if b == b'\n' {
                count.lines += 1;
            }

            if is_space(b) {
                in_word = false;
            } else if !in_word {
                count.words += 1;
                in_word = true;
            }
        }
    }
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t' | 0x0b | 0x0c)
}

fn print_count(count: Count, name: Option<&[u8]>) {
    print_usize(count.lines);
    write_all(1, b" ");
    print_usize(count.words);
    write_all(1, b" ");
    print_usize(count.bytes);

    if let Some(name) = name {
        write_all(1, b" ");
        write_all(1, name);
    }

    println!();
}

fn print_usize(mut n: usize) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();

    if n == 0 {
        write_all(1, b"0");
        return;
    }

    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    write_all(1, &buf[i..]);
}
