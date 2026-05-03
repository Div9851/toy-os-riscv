use crate::cpu;
use crate::proc;
use crate::proc::Trapframe;
use crate::{print, println};

pub const SYS_EXIT: usize = 93;
pub const SYS_PUTC: usize = 1024; // テスト用

pub fn syscall() {
    let p = unsafe { &mut *proc::myproc() };
    let tf = unsafe { &mut *p.trapframe };
    let num = tf.a7 as usize;
    let ret: i64 = match num {
        SYS_EXIT => sys_exit(tf),
        SYS_PUTC => sys_putc(tf),
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

fn sys_putc(tf: &Trapframe) -> i64 {
    print!("{}", (tf.a0 & 0xff) as u8 as char);
    0
}
