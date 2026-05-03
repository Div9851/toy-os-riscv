use crate::memlayout::VirtAddr;
use crate::println;
use crate::proc;
use crate::proc::Trapframe;
use crate::vm::{CopyError, copyin};
use crate::{console, cpu};

pub const SYS_WRITE: usize = 64;
pub const SYS_EXIT: usize = 93;

const EBADF: i64 = -9;
const EFAULT: i64 = -14;

fn errno_of(e: CopyError) -> i64 {
    match e {
        CopyError::Fault => EFAULT,
    }
}

pub fn syscall() {
    let p = unsafe { &mut *proc::myproc() };
    let tf = unsafe { &mut *p.trapframe };
    let num = tf.a7 as usize;
    let ret: i64 = match num {
        SYS_WRITE => sys_write(tf),
        SYS_EXIT => sys_exit(tf),
        _ => {
            println!("unknown syscall {}", num);
            -38 /* ENOSYS */
        }
    };
    tf.a0 = ret as u64;
    tf.epc += 4;
}

fn sys_exit(tf: &Trapframe) -> ! {
    let code = tf.a0 as i32;
    println!("[kernel] proc exited with code {}", code);
    cpu::intr_on();
    loop {
        unsafe { core::arch::asm!("wfi") }
    }
}

fn sys_write(tf: &Trapframe) -> i64 {
    let p = unsafe { &mut *proc::myproc() };

    let fd = tf.a0 as i32;
    let buf = tf.a1 as usize;
    let len = tf.a2 as usize;

    if !(fd == 1 || fd == 2) {
        return EBADF;
    }

    let mut chunk = [0u8; 128];
    let mut off = 0;
    while off < len {
        let n = core::cmp::min(128, len - off);
        if let Err(e) = copyin(p.pagetable, &mut chunk[..n], VirtAddr(buf + off)) {
            return errno_of(e);
        }
        console::write_bytes(&chunk[..n]);
        off += n;
    }
    len as i64
}
