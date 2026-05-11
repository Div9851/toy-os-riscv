use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{
    cpu::{self, intr_get},
    file::{self, CONSOLE_MAJOR, File, FileKind, NOFILE},
    fs::InodeRef,
    kalloc::{kalloc, kalloc_zeroed, kfree},
    loader,
    memlayout::{PGSIZE, PhysAddr, VirtAddr},
    spinlock::RawSpinlock,
    trap,
    vm::{self, PageTable, copyout, uvmcopy_stack},
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

static mut INITPROC: *mut Process = core::ptr::null_mut();

/// One process-table slot.
///
/// Most fields are protected by `lock`. This module currently exposes the
/// fields to low-level trap/syscall code, but the ownership rule is still the
/// xv6-style one: state transitions, context ownership, and kernel-stack
/// ownership are coordinated while holding `lock`.
pub struct Process {
    pub lock: RawSpinlock,
    pub state: ProcessState,
    pub pid: usize,
    pub context: Context,

    pub parent: *mut Process,
    pub xstate: i32,
    pub chan: usize,

    pub pagetable: *mut PageTable,
    pub trapframe: *mut Trapframe,
    pub sz: usize,
    pub kstack: usize,

    pub ofile: [*mut File; NOFILE],
}

impl Process {
    pub const fn unused() -> Self {
        Self {
            lock: RawSpinlock::new(),
            state: ProcessState::Unused,
            pid: 0,
            context: Context::zero(),
            parent: core::ptr::null_mut(),
            xstate: 0,
            chan: 0,
            pagetable: core::ptr::null_mut(),
            trapframe: core::ptr::null_mut(),
            sz: 0,
            kstack: 0,
            ofile: [core::ptr::null_mut(); NOFILE],
        }
    }
}

pub fn myproc() -> *mut Process {
    (*cpu::mycpu()).proc
}

/// Allocate a process slot and its basic kernel resources.
///
/// On success, returns with `p.lock` held. The caller must finish initializing
/// the process, set its state (typically `Runnable`), and release `p.lock`.
/// On failure, releases any lock it acquired and returns `None`.
pub fn allocproc() -> Option<*mut Process> {
    let base = core::ptr::addr_of_mut!(PROCS) as *mut Process;

    for i in 0..NPROC {
        let p_ptr = unsafe { base.add(i) };

        unsafe {
            (*p_ptr).lock.acquire();
        }

        if unsafe { (*p_ptr).state } != ProcessState::Unused {
            unsafe {
                (*p_ptr).lock.release();
            }
            continue;
        }

        let p = unsafe { &mut *p_ptr };

        p.pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
        p.state = ProcessState::Used;
        p.context = Context::zero();
        p.parent = core::ptr::null_mut();
        p.xstate = 0;
        p.chan = 0;
        p.pagetable = core::ptr::null_mut();
        p.trapframe = core::ptr::null_mut();
        p.sz = 0;
        p.kstack = 0;
        p.ofile = [core::ptr::null_mut(); NOFILE];

        let tf_pa = match kalloc_zeroed() {
            Some(pa) => pa,
            None => {
                freeproc(p);
                p.lock.release();
                return None;
            }
        };
        let trapframe = tf_pa.as_mut_ptr::<Trapframe>();
        p.trapframe = trapframe;

        let pagetable = match vm::proc_pagetable(tf_pa) {
            Some(pa) => pa,
            None => {
                freeproc(p);
                p.lock.release();
                return None;
            }
        };
        p.pagetable = pagetable;

        let kstack_pa = match kalloc() {
            Some(pa) => pa,
            None => {
                freeproc(p);
                p.lock.release();
                return None;
            }
        };
        let kstack = kstack_pa.as_usize();
        p.kstack = kstack;

        return Some(p);
    }

    None
}

/// Free resources owned by `p`.
///
/// Caller must hold `p.lock`.
fn freeproc(p: &mut Process) {
    assert!(p.lock.holding(), "freeproc: p.lock not held");
    if !p.trapframe.is_null() {
        kfree(PhysAddr(p.trapframe as usize));
    }
    if !p.pagetable.is_null() {
        vm::proc_freepagetable(p.pagetable, p.sz);
    }
    if p.kstack != 0 {
        kfree(PhysAddr(p.kstack));
    }
    for fd in 0..NOFILE {
        if !p.ofile[fd].is_null() {
            file::close(unsafe { &mut *p.ofile[fd] });
            p.ofile[fd] = core::ptr::null_mut();
        }
    }

    p.trapframe = core::ptr::null_mut();
    p.pagetable = core::ptr::null_mut();
    p.sz = 0;
    p.kstack = 0;
    p.pid = 0;
    p.context = Context::zero();
    p.state = ProcessState::Unused;
    p.parent = core::ptr::null_mut();
    p.xstate = 0;
    p.chan = 0;
}

pub fn userinit() -> *mut Process {
    let p = allocproc().expect("userinit: allocproc");
    unsafe {
        if INITPROC.is_null() {
            INITPROC = p;
        }
    }
    let p = unsafe { &mut *p };
    let image =
        loader::load_elf(unsafe { &mut *p.pagetable }, loader::INIT_ELF).expect("exec init");
    p.sz = image.sz;
    unsafe {
        (*p.trapframe).epc = image.entry as u64;
        (*p.trapframe).sp = image.sp as u64;
    }
    p.context.ra = forkret as *const () as u64;
    p.context.sp = (p.kstack + PGSIZE) as u64;
    p.state = ProcessState::Runnable;
    p.ofile[0] = open_console(true, false).expect("userinit: stdin");
    p.ofile[1] = open_console(false, true).expect("userinit: stdout");
    p.ofile[2] = open_console(false, true).expect("userinit: stderr");
    p.lock.release();
    p
}

fn open_console(readable: bool, writable: bool) -> Option<*mut File> {
    let fp = file::alloc()?;
    let f = unsafe { &mut *fp };
    f.readable = readable;
    f.writable = writable;
    f.kind = FileKind::Device {
        major: CONSOLE_MAJOR,
    };
    Some(fp)
}

pub const NPROC: usize = 16;
static mut PROCS: [Process; NPROC] = [const { Process::unused() }; NPROC];
static NEXT_PID: AtomicUsize = AtomicUsize::new(1);
static WAIT_LOCK: RawSpinlock = RawSpinlock::new();

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Unused,
    Used,
    Runnable,
    Running,
    Zombie,
    Sleeping,
}

/// Saved kernel context used by `swtch.S`.
///
/// This is not a trapframe. `swtch` only preserves the callee-saved register
/// set required by the RISC-V psABI plus `ra` and `sp`; caller-saved registers
/// are allowed to be clobbered across the call.
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

/// First return point for a newly scheduled process.
///
/// `allocproc` initializes `p.context.ra` to this function. The scheduler
/// switches to the process with `p.lock` still held; `forkret` releases that
/// lock before entering the normal user-return path.
extern "C" fn forkret() -> ! {
    let p = myproc();
    assert!(!p.is_null(), "forkret: no proc");

    unsafe {
        (*p).lock.release();
    }

    trap::usertrapret();
}

/// Run the per-CPU scheduler loop.
///
/// The scheduler scans for `Runnable` processes. For a selected process it
/// holds `p.lock`, marks it `Running`, assigns `cpu.proc`, and switches to the
/// process kernel context. The process returns here only after calling `sched`
/// with its state changed away from `Running`; the scheduler then clears
/// `cpu.proc` and releases `p.lock`.
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

/// Switch from the current process kernel context back to the scheduler.
///
/// Caller must hold `p.lock`, must have already changed `p.state` to a
/// non-`Running` state, and interrupts must be disabled by exactly one
/// `push_off` nesting level. The process lock is intentionally held across
/// `swtch`; the scheduler releases it after regaining ownership of the process.
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

/// Voluntarily give up the CPU.
///
/// The current process is marked `Runnable` and control returns to the
/// scheduler. When this process is scheduled again, execution resumes after
/// `sched` and the process lock is released here.
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

/// Terminate the current process.
///
/// Records the exit status, reparents children to init, wakes the parent, marks
/// the process `Zombie`, and switches to the scheduler. The zombie process's
/// resources remain allocated until its parent reaps it with `wait`.
pub fn exit(code: i32) -> ! {
    let p = cpu::mycpu().proc;
    assert!(!p.is_null(), "exit: no proc");

    WAIT_LOCK.acquire();

    reparent(p);

    unsafe {
        wakeup((*p).parent as usize);
    }

    unsafe {
        (*p).lock.acquire();
        (*p).xstate = code;
        (*p).state = ProcessState::Zombie;
    }

    WAIT_LOCK.release();

    sched();
    unreachable!()
}

fn reparent(parent: *mut Process) {
    let initproc = unsafe { INITPROC };
    if initproc.is_null() || parent == initproc {
        return;
    }

    let base = core::ptr::addr_of_mut!(PROCS) as *mut Process;

    let mut need_wakeup = false;

    for i in 0..NPROC {
        let child = unsafe { base.add(i) };

        unsafe {
            (*child).lock.acquire();

            if (*child).parent == parent {
                (*child).parent = initproc;
                need_wakeup = true;
            }

            (*child).lock.release();
        }
    }

    if need_wakeup {
        wakeup(initproc as usize);
    }
}

/// Create a child process by deep-copying the current process address space.
///
/// On success the child is made `Runnable`; the parent receives the child pid
/// and the child will observe return value 0 in `a0` after it enters user mode.
/// On failure, partially allocated child resources are freed.
pub fn fork() -> Option<usize> {
    let parent = unsafe { &mut *myproc() };

    let child_ptr = allocproc()?; // hold lock
    let child = unsafe { &mut *child_ptr };
    child.parent = parent;

    if vm::uvmcopy(
        unsafe { &mut *parent.pagetable },
        unsafe { &mut *child.pagetable },
        parent.sz,
    )
    .is_none()
    {
        freeproc(child);
        child.lock.release();
        return None;
    }

    child.sz = parent.sz;

    unsafe {
        core::ptr::copy_nonoverlapping(parent.trapframe, child.trapframe, 1);
        (*child.trapframe).a0 = 0; // child returns 0 from fork
    }

    if uvmcopy_stack(unsafe { &mut *parent.pagetable }, unsafe {
        &mut *child.pagetable
    })
    .is_none()
    {
        freeproc(child);
        child.lock.release();
        return None;
    }

    child.context.ra = forkret as *const () as u64;
    child.context.sp = (child.kstack + PGSIZE) as u64;

    for fd in 0..NOFILE {
        let f = parent.ofile[fd];
        if !f.is_null() {
            file::dup(unsafe { &mut *f });
            child.ofile[fd] = f;
        }
    }

    let pid = child.pid;
    child.state = ProcessState::Runnable;
    child.lock.release();

    Some(pid)
}

/// Wait for a child process to become zombie and reap it.
///
/// If `status_va != 0`, the child's exit status is copied to that user address.
/// `WAIT_LOCK` protects the parent/child scan plus sleep against lost wakeups
/// with `exit`.
pub fn wait(status_va: usize) -> isize {
    let parent = myproc();
    assert!(!parent.is_null(), "wait: no proc");

    WAIT_LOCK.acquire();

    loop {
        let mut have_child = false;
        let base = core::ptr::addr_of_mut!(PROCS) as *mut Process;

        for i in 0..NPROC {
            let child = unsafe { base.add(i) };

            unsafe {
                (*child).lock.acquire();

                if (*child).parent == parent {
                    have_child = true;

                    if (*child).state == ProcessState::Zombie {
                        let pid = (*child).pid;
                        let xstate = (*child).xstate;

                        if status_va != 0 {
                            let bytes = xstate.to_ne_bytes();
                            if vm::copyout(&mut *(*parent).pagetable, VirtAddr(status_va), &bytes)
                                .is_none()
                            {
                                (*child).lock.release();
                                return -1;
                            }
                        }

                        freeproc(&mut *child);
                        (*child).lock.release();
                        WAIT_LOCK.release();
                        return pid as isize;
                    }
                }

                (*child).lock.release();
            }
        }

        if !have_child {
            WAIT_LOCK.release();
            return -1;
        }

        sleep(parent as usize, &WAIT_LOCK);
    }
}

/// Sleep on `chan`, atomically releasing `lock`.
///
/// The caller must hold `lock`. This function acquires the process lock before
/// releasing `lock`, which closes the lost-wakeup window between checking a
/// condition and marking the process `Sleeping`. On return, `lock` is held
/// again.
pub fn sleep(chan: usize, lock: &RawSpinlock) {
    let p = myproc();
    assert!(!p.is_null(), "sleep: no proc");

    unsafe {
        (*p).lock.acquire();
        lock.release();

        (*p).chan = chan;
        (*p).state = ProcessState::Sleeping;

        sched();

        (*p).chan = 0;

        (*p).lock.release();
        lock.acquire();
    }
}

/// Wake all processes sleeping on `chan`.
///
/// Callers normally hold the condition lock associated with `chan`, so a waiter
/// cannot miss a wakeup between checking the condition and entering `sleep`.
pub fn wakeup(chan: usize) {
    let base = core::ptr::addr_of_mut!(PROCS) as *mut Process;

    for i in 0..NPROC {
        let p = unsafe { base.add(i) };

        unsafe {
            (*p).lock.acquire();

            if (*p).state == ProcessState::Sleeping && (*p).chan == chan {
                (*p).state = ProcessState::Runnable;
            }

            (*p).lock.release();
        }
    }
}

pub const MAXARG: usize = 16;
pub const MAXARGLEN: usize = 128;

/// Kernel-owned copy of exec argv.
///
/// `args[i]` contains a NUL-terminated argument copied from the old user
/// address space. `lens[i]` is the length excluding the trailing NUL.
/// Valid entries are `0..argc`.
pub struct KernelArgs {
    pub argc: usize,
    pub args: [[u8; MAXARGLEN]; MAXARG],
    pub lens: [usize; MAXARG],
}

/// Replace the current process image with `elf`.
///
/// The old address space is kept intact until the new page table has been
/// created and the ELF image has been loaded successfully. On failure, the new
/// partial address space is freed and the current process continues unchanged.
pub fn exec_from_inode(inode: InodeRef, argv: &KernelArgs) -> Option<()> {
    let p = unsafe { &mut *myproc() };

    let new_pt = vm::proc_pagetable(PhysAddr(p.trapframe as usize))?;
    let image = match loader::load_elf_from_inode(unsafe { &mut *new_pt }, inode) {
        Some(image) => image,
        None => {
            vm::proc_freepagetable(new_pt, 0);
            return None;
        }
    };
    let (sp, argv_va) = match push_argv(unsafe { &mut *new_pt }, image.sp, argv) {
        Some(v) => v,
        None => {
            vm::proc_freepagetable(new_pt, image.sz);
            return None;
        }
    };

    let old_pt = p.pagetable;
    let old_sz = p.sz;

    p.pagetable = new_pt;
    p.sz = image.sz;

    unsafe {
        (*p.trapframe).a0 = argv.argc as u64;
        (*p.trapframe).a1 = argv_va as u64;
        (*p.trapframe).epc = image.entry as u64;
        (*p.trapframe).sp = sp as u64;
    }

    vm::proc_freepagetable(old_pt, old_sz);
    Some(())
}

fn align_down(x: usize, align: usize) -> usize {
    x & !(align - 1)
}

/// Copy exec arguments onto the new user stack.
///
/// Places NUL-terminated argument strings and an argv pointer array in the
/// stack page already created by the loader. Returns the final stack pointer and
/// the user virtual address of argv. The final stack pointer is 16-byte aligned.
fn push_argv(pt: &mut PageTable, stack_top: usize, kargs: &KernelArgs) -> Option<(usize, usize)> {
    let stack_bottom = stack_top - PGSIZE;
    let mut sp = stack_top;
    let mut arg_ptrs = [0usize; MAXARG + 1];

    for i in (0..kargs.argc).rev() {
        let len = kargs.lens[i] + 1; // include NUL
        sp = sp.checked_sub(len)?;
        if sp < stack_bottom {
            return None;
        }

        copyout(pt, VirtAddr(sp), &kargs.args[i][..len])?;

        arg_ptrs[i] = sp;
    }

    let argv_bytes = (kargs.argc + 1) * core::mem::size_of::<usize>();
    sp = sp.checked_sub(argv_bytes)?;
    sp = align_down(sp, 16);
    if sp < stack_bottom {
        return None;
    }

    let argv_va = sp;

    for i in 0..=kargs.argc {
        let bytes = arg_ptrs[i].to_ne_bytes();
        copyout(
            pt,
            VirtAddr(argv_va + i * core::mem::size_of::<usize>()),
            &bytes,
        )?;
    }

    Some((sp, argv_va))
}
