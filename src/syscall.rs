use crate::file;
use crate::kalloc::kalloc_zeroed;
use crate::kalloc::kfree;
use crate::loader;
use crate::memlayout::MAXVA;
use crate::memlayout::VirtAddr;
use crate::println;
use crate::proc;
use crate::proc::KernelArgs;
use crate::proc::Trapframe;
use crate::vm::{PageTable, copyin, copyinstr, copyout};

pub const SYS_FORK: usize = 1;
pub const SYS_EXIT: usize = 2;
pub const SYS_WAIT: usize = 3;
pub const SYS_READ: usize = 5;
pub const SYS_EXEC: usize = 7;
pub const SYS_GETPID: usize = 11;
pub const SYS_WRITE: usize = 16;

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
    println!("[kernel] proc exited with code {}", code);
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

fn sys_exec(tf: &Trapframe) -> SyscallResult {
    let p = unsafe { &mut *proc::myproc() };

    let path_va = tf.a0 as usize;
    let mut path: [u8; 128] = [0; 128];
    let path_len = match copyinstr(unsafe { &mut *p.pagetable }, &mut path, VirtAddr(path_va)) {
        Some(len) => len,
        None => return SyscallResult::Return(SYSERR),
    };

    let argv_va = tf.a1 as usize;

    let kargs_pa = match kalloc_zeroed() {
        Some(pa) => pa,
        None => return SyscallResult::Return(SYSERR),
    };
    let kargs = unsafe { &mut *kargs_pa.as_mut_ptr::<KernelArgs>() };
    if copy_argv(unsafe { &mut *p.pagetable }, argv_va, kargs).is_none() {
        kfree(kargs_pa);
        return SyscallResult::Return(SYSERR);
    };

    let name = &path[..path_len];

    for program in loader::PROGRAMS.iter() {
        if name == program.name.as_bytes() {
            let ret = match proc::exec(program.elf, kargs) {
                Some(_) => SyscallResult::Replaced,
                None => SyscallResult::Return(SYSERR),
            };
            kfree(kargs_pa);
            return ret;
        }
    }

    kfree(kargs_pa);
    SyscallResult::Return(SYSERR)
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
