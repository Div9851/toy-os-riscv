use crate::{console, spinlock::RawSpinlock};

pub const NFILE: usize = 100;
pub const NOFILE: usize = 16;
static mut FTABLE: [File; NFILE] = [const { File::unused() }; NFILE];
static FTABLE_LOCK: RawSpinlock = RawSpinlock::new();
pub type Major = u16;

pub const CONSOLE_MAJOR: Major = 1;

pub struct File {
    pub refcnt: usize,
    pub readable: bool,
    pub writable: bool,
    pub kind: FileKind,
}

pub enum FileKind {
    None,
    Device { major: Major },
}

impl File {
    const fn unused() -> Self {
        Self {
            refcnt: 0,
            readable: false,
            writable: false,
            kind: FileKind::None,
        }
    }
}

pub fn alloc() -> Option<*mut File> {
    FTABLE_LOCK.acquire();
    for i in 0..NFILE {
        let f = unsafe { &mut FTABLE[i] };
        if f.refcnt == 0 {
            *f = File::unused();
            f.refcnt = 1;

            let ptr = f as *mut File;
            FTABLE_LOCK.release();
            return Some(ptr);
        }
    }

    FTABLE_LOCK.release();
    None
}

pub fn read(f: &mut File, dst: &mut [u8]) -> isize {
    if !f.readable {
        return -1;
    }

    match f.kind {
        FileKind::Device {
            major: CONSOLE_MAJOR,
        } => console::read(dst),
        _ => -1,
    }
}

pub fn write(f: &mut File, src: &[u8]) -> isize {
    if !f.writable {
        return -1;
    }

    match f.kind {
        FileKind::Device {
            major: CONSOLE_MAJOR,
        } => {
            console::write_bytes(src);
            src.len() as isize
        }
        _ => -1,
    }
}

pub fn dup(f: &mut File) {
    FTABLE_LOCK.acquire();
    assert!(f.refcnt > 0, "dup: invalid refcnt");
    f.refcnt += 1;
    FTABLE_LOCK.release();
}

pub fn close(f: &mut File) {
    FTABLE_LOCK.acquire();
    assert!(f.refcnt > 0, "close: invalid refcnt");
    f.refcnt -= 1;
    if f.refcnt == 0 {
        *f = File::unused();
    }
    FTABLE_LOCK.release();
}
