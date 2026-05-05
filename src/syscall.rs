use crate::console;
use crate::memlayout::MAXVA;
use crate::memlayout::VirtAddr;
use crate::println;
use crate::proc;
use crate::proc::Trapframe;
use crate::vm::copyin;

pub const SYS_FORK: usize = 1;
pub const SYS_EXIT: usize = 2;
pub const SYS_WAIT: usize = 3;
pub const SYS_READ: usize = 5;
pub const SYS_GETPID: usize = 11;
pub const SYS_WRITE: usize = 16;

const SYSERR: i64 = -1;

pub fn syscall() {
    let p = unsafe { &mut *proc::myproc() };
    let tf = unsafe { &mut *p.trapframe };
    let num = tf.a7 as usize;
    if num == SYS_EXIT {
        sys_exit(tf);
    }
    let ret: i64 = match num {
        SYS_FORK => sys_fork(),
        SYS_WRITE => sys_write(tf),
        SYS_WAIT => sys_wait(tf),
        SYS_GETPID => sys_getpid(),
        _ => {
            println!("unknown syscall {}", num);
            SYSERR
        }
    };
    tf.a0 = ret as u64;
}

fn sys_exit(tf: &Trapframe) -> ! {
    let code = tf.a0 as i32;
    println!("[kernel] proc exited with code {}", code);
    proc::exit(code);
}

fn sys_write(tf: &Trapframe) -> i64 {
    let p = unsafe { &mut *proc::myproc() };

    let fd = tf.a0 as i32;
    let buf = tf.a1 as usize;
    let len = tf.a2 as usize;

    if !(fd == 1 || fd == 2) {
        return SYSERR;
    }

    if len == 0 {
        return 0;
    }

    if len > isize::MAX as usize {
        return SYSERR;
    }

    let end = match buf.checked_add(len) {
        Some(end) => end,
        None => return SYSERR,
    };

    if end > MAXVA {
        return SYSERR;
    }

    let mut chunk = [0u8; 128];
    let mut off = 0;
    while off < len {
        let n = core::cmp::min(128, len - off);
        if copyin(
            unsafe { &mut *p.pagetable },
            &mut chunk[..n],
            VirtAddr(buf + off),
        )
        .is_none()
        {
            return SYSERR;
        }
        console::write_bytes(&chunk[..n]);
        off += n;
    }
    len as i64
}

fn sys_fork() -> i64 {
    match proc::fork() {
        Some(pid) => pid as i64,
        None => SYSERR,
    }
}

fn sys_wait(tf: &Trapframe) -> i64 {
    proc::wait(tf.a0 as usize) as i64
}

fn sys_getpid() -> i64 {
    let p = unsafe { &mut *proc::myproc() };
    p.pid as i64
}
