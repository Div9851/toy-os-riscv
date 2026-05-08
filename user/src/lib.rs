#![no_std]

use core::arch::asm;
use core::fmt::{self, Write};
use core::panic::PanicInfo;

// NOTE: keep in sync with /src/syscall.rs

pub const SYS_FORK: usize = 1;
pub const SYS_EXIT: usize = 2;
pub const SYS_WAIT: usize = 3;
pub const SYS_READ: usize = 5;
pub const SYS_EXEC: usize = 7;
pub const SYS_GETPID: usize = 11;
pub const SYS_OPEN: usize = 15;
pub const SYS_WRITE: usize = 16;
pub const SYS_CLOSE: usize = 21;

pub const O_RDONLY: i32 = 0;

#[inline]
pub unsafe fn syscall6(
    num: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
) -> i64 {
    let ret: i64;
    unsafe {
        asm!(
            "ecall",
            in("a7") num,
            inlateout("a0") a0 => ret,
            in("a1") a1,
            in("a2") a2,
            in("a3") a3,
            in("a4") a4,
            in("a5") a5,
            options(nostack),
        );
    }
    ret
}

#[inline]
pub fn write(fd: i32, buf: &[u8]) -> isize {
    let ret = unsafe {
        syscall6(
            SYS_WRITE,
            fd as usize,
            buf.as_ptr() as usize,
            buf.len(),
            0,
            0,
            0,
        )
    };
    ret as isize
}

pub fn write_all(fd: i32, mut buf: &[u8]) -> isize {
    while !buf.is_empty() {
        let n = write(fd, buf);
        if n < 0 {
            return n;
        }
        if n == 0 {
            return -1;
        }
        buf = &buf[n as usize..];
    }
    0
}

#[inline]
pub fn exit(code: i32) -> ! {
    unsafe {
        syscall6(SYS_EXIT, code as usize, 0, 0, 0, 0, 0);
    }
    loop {}
}

#[inline]
pub fn fork() -> isize {
    unsafe { syscall6(SYS_FORK, 0, 0, 0, 0, 0, 0) as isize }
}

#[inline]
pub fn wait(status: &mut i32) -> isize {
    unsafe { syscall6(SYS_WAIT, status as *mut i32 as usize, 0, 0, 0, 0, 0) as isize }
}

#[inline]
pub fn getpid() -> isize {
    unsafe { syscall6(SYS_GETPID, 0, 0, 0, 0, 0, 0) as isize }
}

#[inline]
pub fn read(fd: i32, buf: &mut [u8]) -> isize {
    unsafe {
        syscall6(
            SYS_READ,
            fd as usize,
            buf.as_mut_ptr() as usize,
            buf.len(),
            0,
            0,
            0,
        ) as isize
    }
}

#[inline]
pub fn execv(path: &[u8], argv: &[*const u8]) -> isize {
    unsafe {
        syscall6(
            SYS_EXEC,
            path.as_ptr() as usize,
            argv.as_ptr() as usize,
            0,
            0,
            0,
            0,
        ) as isize
    }
}

#[inline]
pub fn open(path: &[u8], flags: i32) -> isize {
    unsafe { syscall6(SYS_OPEN, path.as_ptr() as usize, flags as usize, 0, 0, 0, 0) as isize }
}

#[inline]
pub fn close(fd: i32) -> isize {
    unsafe { syscall6(SYS_CLOSE, fd as usize, 0, 0, 0, 0, 0) as isize }
}

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let ret = write_all(1, s.as_bytes());
        if ret < 0 {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments<'_>) {
    let _ = Stdout.write_fmt(args);
}

#[macro_export]
macro_rules! print {
      ($($arg:tt)*) => {
          $crate::_print(core::format_args!($($arg)*))
      };
  }

#[macro_export]
macro_rules! println {
      () => {
          $crate::print!("\n")
      };
      ($fmt:expr) => {
          $crate::print!(core::concat!($fmt, "\n"))
      };
      ($fmt:expr, $($arg:tt)*) => {
          $crate::print!(core::concat!($fmt, "\n"), $($arg)*)
      };
  }

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(255);
}

pub struct Args {
    argc: usize,
    argv: *const *const u8,
}

impl Args {
    pub const fn new(argc: usize, argv: *const *const u8) -> Self {
        Self { argc, argv }
    }

    pub fn len(&self) -> usize {
        self.argc
    }

    pub fn get(&self, i: usize) -> Option<&[u8]> {
        if i >= self.argc {
            return None;
        }

        let p = unsafe { *self.argv.add(i) };
        if p.is_null() {
            return None;
        }

        let len = unsafe { cstr_len(p) };
        Some(unsafe { core::slice::from_raw_parts(p, len) })
    }
}

unsafe fn cstr_len(p: *const u8) -> usize {
    let mut len = 0;
    unsafe {
        while *p.add(len) != 0 {
            len += 1;
        }
    }
    len
}
