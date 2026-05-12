use crate::{
    console,
    fs::{self, InodeRef},
    spinlock::RawSpinlock,
};

pub const NFILE: usize = 100;
pub const NOFILE: usize = 16;
static mut FTABLE: [File; NFILE] = [const { File::unused() }; NFILE];
static FTABLE_LOCK: RawSpinlock = RawSpinlock::new();

pub const CONSOLE_MAJOR: u16 = 1;

pub struct File {
    pub refcnt: usize,
    pub readable: bool,
    pub writable: bool,
    pub kind: FileKind,
}

pub enum FileKind {
    None,
    Device {
        major: u16,
    },
    Inode {
        inode: InodeRef,
        off: usize,
        append: bool,
    },
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

    match &mut f.kind {
        FileKind::Device {
            major: CONSOLE_MAJOR,
        } => console::read(dst),
        FileKind::Inode { inode, off, .. } => {
            let n = fs::readi(*inode, *off, dst);
            if n > 0 {
                *off += n as usize;
            }
            n
        }
        _ => -1,
    }
}

pub fn write(f: &mut File, src: &[u8]) -> isize {
    if !f.writable {
        return -1;
    }

    match &mut f.kind {
        FileKind::Device {
            major: CONSOLE_MAJOR,
        } => {
            console::write_bytes(src);
            src.len() as isize
        }
        FileKind::Inode { inode, off, append } => {
            if *append {
                let (n, new_off) = fs::appendi(*inode, src);
                if n > 0 {
                    *off = new_off;
                }
                return n;
            }

            let n = fs::writei(*inode, *off, src);
            if n > 0 {
                *off += n as usize;
            }
            n
        }
        _ => -1,
    }
}

pub fn stat(f: &File) -> Option<fs::Stat> {
    match f.kind {
        FileKind::Inode { inode, .. } => Some(fs::stati(inode)),
        _ => None,
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
        let kind = core::mem::replace(&mut f.kind, FileKind::None);
        f.readable = false;
        f.writable = false;
        FTABLE_LOCK.release();

        if let FileKind::Inode { inode, .. } = kind {
            fs::iput(inode);
        }

        return;
    }
    FTABLE_LOCK.release();
}
