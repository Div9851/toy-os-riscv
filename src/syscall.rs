use crate::file;
use crate::file::File;
use crate::file::FileKind;
use crate::fs;
use crate::fs::InodeType;
use crate::kalloc::kalloc_zeroed;
use crate::kalloc::kfree;
use crate::memlayout::MAXVA;
use crate::memlayout::PGSIZE;
use crate::memlayout::USER_STACK;
use crate::memlayout::VirtAddr;
use crate::println;
use crate::proc;
use crate::proc::KernelArgs;
use crate::proc::Process;
use crate::proc::Trapframe;
use crate::vm::uvmalloc;
use crate::vm::{PageTable, copyin, copyinstr, copyout};

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

const O_RDONLY: usize = 0x000;
const O_WRONLY: usize = 0x001;
const O_RDWR: usize = 0x002;
const O_CREATE: usize = 0x200;
const O_TRUNC: usize = 0x400;
const O_APPEND: usize = 0x800;

const SYSERR: i64 = -1;

enum SyscallResult {
    Return(i64),
    Replaced,
}

pub fn syscall() {
    let p = unsafe { &mut *proc::myproc() };
    let tf = unsafe { &mut *p.trapframe };
    let num = tf.a7 as usize;
    if num == SYS_EXIT {
        sys_exit(tf);
    }
    let result = match num {
        SYS_FORK => sys_fork(),
        SYS_WRITE => sys_write(tf),
        SYS_WAIT => sys_wait(tf),
        SYS_GETPID => sys_getpid(),
        SYS_READ => sys_read(tf),
        SYS_EXEC => sys_exec(tf),
        SYS_FSTAT => sys_fstat(tf),
        SYS_CHDIR => sys_chdir(tf),
        SYS_DUP => sys_dup(tf),
        SYS_OPEN => sys_open(tf),
        SYS_MKDIR => sys_mkdir(tf),
        SYS_CLOSE => sys_close(tf),
        SYS_SBRK => sys_sbrk(tf),
        _ => {
            println!("unknown syscall {}", num);
            SyscallResult::Return(SYSERR)
        }
    };
    if let SyscallResult::Return(ret) = result {
        tf.a0 = ret as u64;
    }
}

fn sys_exit(tf: &Trapframe) -> ! {
    let code = tf.a0 as i32;
    proc::exit(code);
}

fn sys_write(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };

    let fd = tf.a0 as usize;
    let buf = tf.a1 as usize;
    let len = tf.a2 as usize;

    if fd >= file::NOFILE {
        return SyscallResult::Return(SYSERR);
    }
    if p.ofile[fd] == core::ptr::null_mut() {
        return SyscallResult::Return(SYSERR);
    }

    let f = unsafe { &mut *p.ofile[fd] };
    if !f.writable {
        return SyscallResult::Return(SYSERR);
    }

    if len == 0 {
        return SyscallResult::Return(0);
    }

    if len > isize::MAX as usize {
        return SyscallResult::Return(SYSERR);
    }

    let end = match buf.checked_add(len) {
        Some(end) => end,
        None => return SyscallResult::Return(SYSERR),
    };

    if end > MAXVA {
        return SyscallResult::Return(SYSERR);
    }

    let mut chunk = [0u8; 128];
    let n = core::cmp::min(128, len);
    if copyin(unsafe { &mut *p.pagetable }, &mut chunk[..n], VirtAddr(buf)).is_none() {
        return SyscallResult::Return(SYSERR);
    }
    let nw = file::write(f, &chunk[..n]);
    if nw < 0 {
        return SyscallResult::Return(SYSERR);
    }
    if nw == 0 {
        return SyscallResult::Return(SYSERR);
    }
    SyscallResult::Return(nw as i64)
}

fn sys_fork() -> SyscallResult {
    match proc::fork() {
        Some(pid) => SyscallResult::Return(pid as i64),
        None => SyscallResult::Return(SYSERR),
    }
}

fn sys_wait(tf: &Trapframe) -> SyscallResult {
    SyscallResult::Return(proc::wait(tf.a0 as usize) as i64)
}

fn sys_getpid() -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };
    SyscallResult::Return(p.pid as i64)
}

fn sys_read(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };

    let fd = tf.a0 as usize;
    let buf = tf.a1 as usize;
    let len = tf.a2 as usize;

    if fd >= file::NOFILE {
        return SyscallResult::Return(SYSERR);
    }
    if p.ofile[fd] == core::ptr::null_mut() {
        return SyscallResult::Return(SYSERR);
    }

    let f = unsafe { &mut *p.ofile[fd] };
    if !f.readable {
        return SyscallResult::Return(SYSERR);
    }

    if len == 0 {
        return SyscallResult::Return(0);
    }

    if len > isize::MAX as usize {
        return SyscallResult::Return(SYSERR);
    }

    let end = match buf.checked_add(len) {
        Some(end) => end,
        None => return SyscallResult::Return(SYSERR),
    };

    if end > MAXVA {
        return SyscallResult::Return(SYSERR);
    }

    let mut kbuf = [0u8; 128];
    let cap = core::cmp::min(kbuf.len(), len);

    let nr = file::read(f, &mut kbuf[..cap]);
    if nr < 0 {
        return SyscallResult::Return(SYSERR);
    }

    let nr = nr as usize;

    if copyout(unsafe { &mut *p.pagetable }, VirtAddr(buf), &kbuf[..nr]).is_none() {
        return SyscallResult::Return(SYSERR);
    }

    SyscallResult::Return(nr as i64)
}

fn sys_fstat(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };

    let fd = tf.a0 as usize;
    let stat_va = tf.a1 as usize;

    if fd >= file::NOFILE {
        return SyscallResult::Return(SYSERR);
    }
    if p.ofile[fd] == core::ptr::null_mut() {
        return SyscallResult::Return(SYSERR);
    }

    let f = unsafe { &mut *p.ofile[fd] };
    let st = match file::stat(f) {
        Some(st) => st,
        None => return SyscallResult::Return(SYSERR),
    };

    let st_bytes = unsafe {
        core::slice::from_raw_parts(
            core::ptr::addr_of!(st) as *const u8,
            core::mem::size_of::<fs::Stat>(),
        )
    };

    if copyout(unsafe { &mut *p.pagetable }, VirtAddr(stat_va), st_bytes).is_none() {
        return SyscallResult::Return(SYSERR);
    }

    SyscallResult::Return(0)
}

fn sys_exec(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };

    let path_va = tf.a0 as usize;
    let mut path: [u8; 128] = [0; 128];
    let path_len = match copyinstr(unsafe { &mut *p.pagetable }, &mut path, VirtAddr(path_va)) {
        Some(len) => len,
        None => return SyscallResult::Return(SYSERR),
    };
    let path = &path[..path_len];

    let inode = match fs::namei(p.cwd, path) {
        Some(inode) => inode,
        None => {
            return SyscallResult::Return(SYSERR);
        }
    };

    let argv_va = tf.a1 as usize;

    let kargs_pa = match kalloc_zeroed() {
        Some(pa) => pa,
        None => {
            fs::iput(inode);
            return SyscallResult::Return(SYSERR);
        }
    };
    let kargs = unsafe { &mut *kargs_pa.as_mut_ptr::<KernelArgs>() };
    if copy_argv(unsafe { &mut *p.pagetable }, argv_va, kargs).is_none() {
        kfree(kargs_pa);
        fs::iput(inode);
        return SyscallResult::Return(SYSERR);
    };

    let ret = match proc::exec_from_inode(inode, kargs) {
        Some(_) => SyscallResult::Replaced,
        None => SyscallResult::Return(SYSERR),
    };

    fs::iput(inode);
    kfree(kargs_pa);
    ret
}

fn sys_chdir(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };

    let path_va = tf.a0 as usize;
    let mut path: [u8; 128] = [0; 128];
    let path_len = match copyinstr(unsafe { &mut *p.pagetable }, &mut path, VirtAddr(path_va)) {
        Some(len) => len,
        None => return SyscallResult::Return(SYSERR),
    };
    let path = &path[..path_len];

    let inode = match fs::namei(p.cwd, path) {
        Some(inode) => inode,
        None => return SyscallResult::Return(SYSERR),
    };

    match fs::inode_type(inode) {
        InodeType::Dir => {
            fs::iput(p.cwd);
            p.cwd = inode;
            SyscallResult::Return(0)
        }
        _ => {
            fs::iput(inode);
            SyscallResult::Return(SYSERR)
        }
    }
}

fn sys_dup(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };
    let fd = tf.a0 as usize;

    if fd >= file::NOFILE {
        return SyscallResult::Return(SYSERR);
    }

    let fp = p.ofile[fd];
    if fp.is_null() {
        return SyscallResult::Return(SYSERR);
    }

    let newfd = match fdalloc(p, fp) {
        Some(fd) => fd,
        None => return SyscallResult::Return(SYSERR),
    };

    file::dup(unsafe { &mut *fp });
    SyscallResult::Return(newfd as i64)
}

const USIZE_BYTES: usize = core::mem::size_of::<usize>();

/// Copy exec argv from the old user address space into `kargs`.
///
/// `kargs` must be zero-initialized by the caller. On success, `argc >= 1`.
fn copy_argv(pt: &mut PageTable, argv_va: usize, kargs: &mut KernelArgs) -> Option<()> {
    if argv_va == 0 {
        return None;
    }

    loop {
        let ptr_va = argv_va.checked_add(kargs.argc * USIZE_BYTES)?;
        let mut ptr_bytes = [0u8; USIZE_BYTES];
        copyin(pt, &mut ptr_bytes, VirtAddr(ptr_va))?;

        let arg_va = u64::from_ne_bytes(ptr_bytes) as usize;

        if arg_va == 0 {
            return if kargs.argc == 0 { None } else { Some(()) };
        }

        if kargs.argc >= proc::MAXARG {
            return None;
        }

        let len = copyinstr(pt, &mut kargs.args[kargs.argc], VirtAddr(arg_va))?;
        kargs.lens[kargs.argc] = len;
        kargs.argc += 1;
    }
}

fn sys_open(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };

    let path_va = tf.a0 as usize;
    let mut path: [u8; 128] = [0; 128];
    let path_len = match copyinstr(unsafe { &mut *p.pagetable }, &mut path, VirtAddr(path_va)) {
        Some(len) => len,
        None => return SyscallResult::Return(SYSERR),
    };
    let path = &path[..path_len];

    let flags = tf.a1 as usize;
    let create = (flags & O_CREATE) != 0;
    let truncate = (flags & O_TRUNC) != 0;
    let append = (flags & O_APPEND) != 0;

    let inode = match if create {
        fs::create(p.cwd, path)
    } else {
        fs::namei(p.cwd, path)
    } {
        Some(inode) => inode,
        None => {
            return SyscallResult::Return(SYSERR);
        }
    };
    let typ = fs::inode_type(inode);

    let readable = (flags & O_WRONLY) == 0;
    let writable = (flags & (O_WRONLY | O_RDWR)) != 0;

    if matches!(typ, InodeType::Dir) && writable {
        fs::iput(inode);
        return SyscallResult::Return(SYSERR);
    }

    if truncate {
        if !matches!(typ, InodeType::File) || !writable {
            fs::iput(inode);
            return SyscallResult::Return(SYSERR);
        }
    }

    let fp = match file::alloc() {
        Some(fp) => fp,
        None => {
            fs::iput(inode);
            return SyscallResult::Return(SYSERR);
        }
    };
    let f = unsafe { &mut *fp };

    f.readable = readable;
    f.writable = writable;

    match typ {
        InodeType::File => {
            f.kind = FileKind::Inode {
                inode,
                off: 0,
                append,
            };
        }
        InodeType::Device { major } => {
            f.kind = FileKind::Device { major };
            fs::iput(inode);
        }
        InodeType::Dir => {
            f.kind = FileKind::Inode {
                inode,
                off: 0,
                append: false,
            };
        }
    }

    let fd = match fdalloc(p, fp) {
        Some(fd) => fd,
        None => {
            file::close(f);
            return SyscallResult::Return(SYSERR);
        }
    };

    if truncate {
        fs::trunc(inode);
    }

    SyscallResult::Return(fd as i64)
}

fn sys_mkdir(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };

    let path_va = tf.a0 as usize;
    let mut path: [u8; 128] = [0; 128];
    let path_len = match copyinstr(unsafe { &mut *p.pagetable }, &mut path, VirtAddr(path_va)) {
        Some(len) => len,
        None => return SyscallResult::Return(SYSERR),
    };
    let path = &path[..path_len];

    match fs::mkdir(p.cwd, path) {
        Some(inode) => {
            fs::iput(inode);
            SyscallResult::Return(0)
        }
        None => SyscallResult::Return(SYSERR),
    }
}

fn fdalloc(p: &mut Process, f: *mut File) -> Option<usize> {
    for fd in 0..file::NOFILE {
        if p.ofile[fd].is_null() {
            p.ofile[fd] = f;
            return Some(fd);
        }
    }
    None
}

fn sys_close(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };
    let fd = tf.a0 as usize;

    if fd >= file::NOFILE {
        return SyscallResult::Return(SYSERR);
    }

    let fp = p.ofile[fd];
    if fp.is_null() {
        return SyscallResult::Return(SYSERR);
    }

    p.ofile[fd] = core::ptr::null_mut();
    file::close(unsafe { &mut *fp });

    SyscallResult::Return(0)
}

fn sys_sbrk(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };
    let n = tf.a0 as isize;
    if n < 0 {
        return SyscallResult::Return(-1);
    }
    let oldsz = p.sz;
    let newsz = match oldsz.checked_add(n as usize) {
        Some(newsz) => newsz,
        None => {
            return SyscallResult::Return(-1);
        }
    };
    let new_page_end = match newsz.checked_add(PGSIZE - 1) {
        Some(v) => v & !(PGSIZE - 1),
        None => return SyscallResult::Return(-1),
    };
    if new_page_end > USER_STACK {
        return SyscallResult::Return(-1);
    }
    if uvmalloc(unsafe { &mut *p.pagetable }, oldsz, newsz).is_none() {
        return SyscallResult::Return(-1);
    }
    p.sz = newsz;

    SyscallResult::Return(oldsz as i64)
}
