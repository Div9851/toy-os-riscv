#![no_std]
#![no_main]

use alloc::vec::Vec;
use core::mem::size_of;
use user::{
    Args, DIRSIZ, Dirent, O_RDONLY, Stat, T_DEVICE, T_DIR, T_FILE, close, exit, fstat, open,
    println, read, write_all,
};

extern crate alloc;

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: usize, argv: *const *const u8) -> ! {
    let args = Args::new(argc, argv);

    if args.len() == 1 {
        ls(b".");
    } else {
        for i in 1..args.len() {
            if let Some(path) = args.get(i) {
                ls(path);
            }
        }
    }

    exit(0);
}

fn ls(path: &[u8]) {
    let fd = open(path, O_RDONLY) as i32;
    if fd < 0 {
        write_all(1, path);
        println!(": open failed");
        return;
    }

    let mut st = empty_stat();
    if fstat(fd, &mut st) < 0 {
        write_all(1, path);
        println!(": fstat failed");
        close(fd);
        return;
    }

    if st.typ != T_DIR {
        print_entry(path, &st);
        close(fd);
        return;
    }

    loop {
        let mut de = Dirent {
            inum: 0,
            name: [0; DIRSIZ],
        };
        let de_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                core::ptr::addr_of_mut!(de) as *mut u8,
                size_of::<Dirent>(),
            )
        };

        let n = read(fd, de_bytes);
        if n == 0 {
            break;
        }
        if n != size_of::<Dirent>() as isize {
            write_all(1, path);
            println!(": short read");
            break;
        }
        if de.inum == 0 {
            continue;
        }

        let name = dirent_name(&de);
        let child_path = join_path(path, name);
        let child_fd = open(&child_path, O_RDONLY) as i32;
        if child_fd < 0 {
            write_all(1, &child_path);
            println!(": open failed");
            continue;
        }

        let mut child_st = empty_stat();
        if fstat(child_fd, &mut child_st) < 0 {
            write_all(1, &child_path);
            println!(": fstat failed");
            close(child_fd);
            continue;
        }

        print_entry(&child_path, &child_st);
        close(child_fd);
    }

    close(fd);
}

fn empty_stat() -> Stat {
    Stat {
        typ: 0,
        ino: 0,
        nlink: 0,
        size: 0,
    }
}

fn dirent_name(de: &Dirent) -> &[u8] {
    let end = de.name.iter().position(|&b| b == 0).unwrap_or(DIRSIZ);
    &de.name[..end]
}

fn join_path(parent: &[u8], name: &[u8]) -> Vec<u8> {
    if parent == b"/" {
        let mut out = Vec::new();
        out.extend_from_slice(b"/");
        out.extend_from_slice(name);
        return out;
    }

    let mut out = parent.to_vec();
    if !out.ends_with(b"/") {
        out.push(b'/');
    }
    out.extend_from_slice(name);
    out
}

fn print_entry(path: &[u8], st: &Stat) {
    write_all(1, path);
    println!(" {} {} {} {}", type_char(st.typ), st.ino, st.nlink, st.size);
}

fn type_char(typ: i16) -> char {
    match typ {
        T_DIR => 'd',
        T_FILE => 'f',
        T_DEVICE => 'c',
        _ => '?',
    }
}
