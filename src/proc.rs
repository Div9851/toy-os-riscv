use crate::{
    cpu::{self, intr_get},
    exec,
    kalloc::{kalloc, kalloc_zeroed},
    memlayout::PGSIZE,
    spinlock::RawSpinlock,
    trap,
    vm::{self, PageTable},
};

#[repr(C)]
pub struct Trapframe {
    /*   0 */ pub kernel_satp: u64,
    /*   8 */ pub kernel_sp: u64,
    /*  16 */ pub kernel_trap: u64,
    /*  24 */ pub epc: u64,
    /*  32 */ pub kernel_hartid: u64,
    /*  40 */ pub ra: u64,
    /*  48 */ pub sp: u64,
    /*  56 */ pub gp: u64,
    /*  64 */ pub tp: u64,
    /*  72 */ pub t0: u64,
    /*  80 */ pub t1: u64,
    /*  88 */ pub t2: u64,
    /*  96 */ pub s0: u64,
    /* 104 */ pub s1: u64,
    /* 112 */ pub a0: u64,
    /* 120 */ pub a1: u64,
    /* 128 */ pub a2: u64,
    /* 136 */ pub a3: u64,
    /* 144 */ pub a4: u64,
    /* 152 */ pub a5: u64,
    /* 160 */ pub a6: u64,
    /* 168 */ pub a7: u64,
    /* 176 */ pub s2: u64,
    /* 184 */ pub s3: u64,
    /* 192 */ pub s4: u64,
    /* 200 */ pub s5: u64,
    /* 208 */ pub s6: u64,
    /* 216 */ pub s7: u64,
    /* 224 */ pub s8: u64,
    /* 232 */ pub s9: u64,
    /* 240 */ pub s10: u64,
    /* 248 */ pub s11: u64,
    /* 256 */ pub t3: u64,
    /* 264 */ pub t4: u64,
    /* 272 */ pub t5: u64,
    /* 280 */ pub t6: u64,
}
const _: () = assert!(core::mem::size_of::<Trapframe>() <= 4096);

pub struct Process {
    pub lock: RawSpinlock,
    pub state: ProcessState,
    pub pid: usize,
    pub context: Context,

    pub pagetable: *mut PageTable,
    pub trapframe: *mut Trapframe,
    pub sz: usize,
    pub kstack: usize,
}

impl Process {
    pub const fn unused() -> Self {
        Self {
            lock: RawSpinlock::new(),
            state: ProcessState::Unused,
            pid: 0,
            context: Context::zero(),
            pagetable: core::ptr::null_mut(),
            trapframe: core::ptr::null_mut(),
            sz: 0,
            kstack: 0,
        }
    }
}

pub fn myproc() -> *mut Process {
    (*cpu::mycpu()).proc
}

pub fn allocproc() -> Option<*mut Process> {
    // スケジューラとの競合を避けるため、interrupt を off にする
    cpu::push_off();

    let base = core::ptr::addr_of_mut!(PROCS) as *mut Process;

    for i in 0..NPROC {
        let p = unsafe { &mut *base.add(i) };
        if p.state == ProcessState::Unused {
            unsafe {
                p.pid = NEXT_PID;
                NEXT_PID += 1;
            }

            let tf_pa = kalloc_zeroed().expect("allocproc: trapframe alloc");
            let trapframe = tf_pa.as_mut_ptr::<Trapframe>();

            let pagetable = vm::proc_pagetable(tf_pa);

            let kstack_pa = kalloc().expect("allocproc: kstack alloc");
            let kstack = kstack_pa.as_usize();

            p.context = Context::zero();
            p.pagetable = pagetable;
            p.trapframe = trapframe;
            p.sz = 0;
            p.kstack = kstack;
            p.state = ProcessState::Used;

            cpu::pop_off();

            return Some(p);
        }
    }

    cpu::pop_off();

    None
}

pub fn userinit() -> *mut Process {
    let p = allocproc().expect("userinit: allocproc");
    let p = unsafe { &mut *p };
    let (entry, sp, sz) =
        exec::exec(unsafe { &mut *p.pagetable }, exec::INIT_ELF).expect("exec init");
    p.sz = sz;
    unsafe {
        (*p.trapframe).epc = entry as u64;
        (*p.trapframe).sp = sp as u64;
    }
    p.context.ra = forkret as *const () as u64;
    p.context.sp = (p.kstack + PGSIZE) as u64;
    p.state = ProcessState::Runnable;
    p
}

pub const NPROC: usize = 16;
static mut PROCS: [Process; NPROC] = [const { Process::unused() }; NPROC];
static mut NEXT_PID: usize = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Unused,
    Used,
    Runnable,
    Running,
    Zombie,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Context {
    pub ra: u64,
    pub sp: u64,
    pub s0: u64,
    pub s1: u64,
    pub s2: u64,
    pub s3: u64,
    pub s4: u64,
    pub s5: u64,
    pub s6: u64,
    pub s7: u64,
    pub s8: u64,
    pub s9: u64,
    pub s10: u64,
    pub s11: u64,
}

impl Context {
    pub const fn zero() -> Self {
        Context {
            ra: 0,
            sp: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
        }
    }
}

unsafe extern "C" {
    fn swtch(old: *mut Context, new: *const Context);
}

extern "C" fn forkret() -> ! {
    let p = myproc();
    assert!(!p.is_null(), "forkret: no proc");

    unsafe {
        (*p).lock.release();
    }

    trap::usertrapret();
}

pub fn scheduler() -> ! {
    loop {
        cpu::intr_on();

        let base = core::ptr::addr_of_mut!(PROCS) as *mut Process;

        for i in 0..NPROC {
            let p = unsafe { base.add(i) };

            unsafe {
                (*p).lock.acquire();
            }

            if unsafe { (*p).state } == ProcessState::Runnable {
                unsafe {
                    (*p).state = ProcessState::Running;
                }
                cpu::mycpu().proc = p;
                unsafe {
                    swtch(
                        core::ptr::addr_of_mut!((*cpu::mycpu_ptr()).context),
                        core::ptr::addr_of!((*p).context),
                    );
                }
                cpu::mycpu().proc = core::ptr::null_mut();
            }

            unsafe {
                (*p).lock.release();
            }
        }
    }
}

pub fn sched() {
    let p = cpu::mycpu().proc;

    assert!(!p.is_null(), "sched: no proc");
    assert!(unsafe { (*p).lock.holding() }, "sched: p.lock not held");
    assert!(
        unsafe { (*p).state != ProcessState::Running },
        "sched: running"
    );
    assert!(!intr_get(), "sched: interrupts enabled");
    assert!(cpu::mycpu().noff == 1, "sched: unexpected noff");

    let intena = cpu::mycpu().intena;
    unsafe {
        swtch(
            core::ptr::addr_of_mut!((*p).context),
            core::ptr::addr_of!((*cpu::mycpu_ptr()).context),
        );
    }
    cpu::mycpu().intena = intena;
}

pub fn yield_cpu() {
    let p = myproc();
    assert!(!p.is_null(), "yield_cpu: no proc");

    unsafe {
        (*p).lock.acquire();
    }
    unsafe {
        (*p).state = ProcessState::Runnable;
    }
    sched();
    unsafe {
        (*p).lock.release();
    }
}

pub fn exit() -> ! {
    let p = cpu::mycpu().proc;
    assert!(!p.is_null(), "exit: no proc");

    unsafe {
        (*p).lock.acquire();
    }
    unsafe {
        (*p).state = ProcessState::Zombie;
    }
    sched();
    unreachable!()
}
