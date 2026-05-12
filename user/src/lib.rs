#![no_std]

use alloc::ffi::CString;
use alloc::vec::Vec;
use core::arch::asm;
use core::fmt::{self, Write};
use core::panic::PanicInfo;

mod allocator;

extern crate alloc;

#[global_allocator]
static ALLOCATOR: allocator::UserAllocator = allocator::UserAllocator::new();

// NOTE: keep in sync with /src/syscall.rs

pub const SYS_FORK: usize = 1;
pub const SYS_EXIT: usize = 2;
pub const SYS_WAIT: usize = 3;
pub const SYS_READ: usize = 5;
pub const SYS_EXEC: usize = 7;
pub const SYS_FSTAT: usize = 8;
pub const SYS_CHDIR: usize = 9;
pub const SYS_DUP: usize = 10;
pub const SYS_GETPID: usize = 11;
pub const SYS_SBRK: usize = 12;
pub const SYS_OPEN: usize = 15;
pub const SYS_WRITE: usize = 16;
pub const SYS_MKDIR: usize = 20;
pub const SYS_CLOSE: usize = 21;

pub const O_RDONLY: i32 = 0x000;
pub const O_WRONLY: i32 = 0x001;
pub const O_RDWR: i32 = 0x002;
pub const O_CREATE: i32 = 0x200;
pub const O_TRUNC: i32 = 0x400;
pub const O_APPEND: i32 = 0x800;

pub const T_DIR: i16 = 1;
pub const T_FILE: i16 = 2;
pub const T_DEVICE: i16 = 3;
pub const DIRSIZ: usize = 14;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Stat {
    pub typ: i16,
    pub ino: u32,
    pub nlink: i16,
    pub size: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Dirent {
    pub inum: u16,
    pub name: [u8; DIRSIZ],
}

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

pub fn exec(path: &[u8], argv: &[&[u8]]) -> isize {
    let path = match CString::new(path) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let args: Vec<CString> = match argv.iter().map(|arg| CString::new(*arg)).collect() {
        Ok(v) => v,
        Err(_) => return -1,
    };

    let mut raw_argv: Vec<*const u8> = args
        .iter()
        .map(|arg| arg.as_bytes_with_nul().as_ptr())
        .collect();

    raw_argv.push(core::ptr::null());

    unsafe { exec_raw(path.as_bytes_with_nul().as_ptr(), raw_argv.as_ptr()) }
}

#[inline]
pub unsafe fn exec_raw(path: *const u8, argv: *const *const u8) -> isize {
    unsafe { syscall6(SYS_EXEC, path as usize, argv as usize, 0, 0, 0, 0) as isize }
}

pub fn chdir(path: &[u8]) -> isize {
    let path = match CString::new(path) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    unsafe { chdir_raw(path.as_bytes_with_nul().as_ptr()) }
}

#[inline]
pub unsafe fn chdir_raw(path: *const u8) -> isize {
    unsafe { syscall6(SYS_CHDIR, path as usize, 0, 0, 0, 0, 0) as isize }
}

pub fn open(path: &[u8], flags: i32) -> isize {
    let path = match CString::new(path) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    unsafe { open_raw(path.as_bytes_with_nul().as_ptr(), flags) }
}

#[inline]
pub unsafe fn open_raw(path: *const u8, flags: i32) -> isize {
    unsafe { syscall6(SYS_OPEN, path as usize, flags as usize, 0, 0, 0, 0) as isize }
}

pub fn mkdir(path: &[u8]) -> isize {
    let path = match CString::new(path) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    unsafe { mkdir_raw(path.as_bytes_with_nul().as_ptr()) }
}

#[inline]
pub unsafe fn mkdir_raw(path: *const u8) -> isize {
    unsafe { syscall6(SYS_MKDIR, path as usize, 0, 0, 0, 0, 0) as isize }
}

#[inline]
pub fn dup(fd: i32) -> isize {
    unsafe { syscall6(SYS_DUP, fd as usize, 0, 0, 0, 0, 0) as isize }
}

#[inline]
pub fn fstat(fd: i32, st: &mut Stat) -> isize {
    unsafe { syscall6(SYS_FSTAT, fd as usize, st as *mut Stat as usize, 0, 0, 0, 0) as isize }
}

#[inline]
pub fn close(fd: i32) -> isize {
    unsafe { syscall6(SYS_CLOSE, fd as usize, 0, 0, 0, 0, 0) as isize }
}

#[inline]
pub fn sbrk(increment: isize) -> isize {
    unsafe { syscall6(SYS_SBRK, increment as usize, 0, 0, 0, 0, 0) as isize }
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
