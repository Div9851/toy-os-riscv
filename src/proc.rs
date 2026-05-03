use crate::{
    cpu,
    kalloc::{kalloc, kalloc_zeroed},
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
    pub pagetable: *mut PageTable,
    pub trapframe: *mut Trapframe,
    pub sz: usize,
    pub kstack: usize,
}

impl Process {
    pub fn new() -> Self {
        let tf_pa = kalloc_zeroed().expect("Process:new: trapframe alloc");
        let trapframe = tf_pa.as_mut_ptr::<Trapframe>();

        let pagetable = vm::proc_pagetable(tf_pa);

        let kstack_pa = kalloc().expect("Process:new: kstack alloc");
        let kstack = kstack_pa.as_usize();

        Self {
            pagetable,
            trapframe,
            sz: 0,
            kstack,
        }
    }
}

pub fn myproc() -> *mut Process {
    (*cpu::mycpu()).proc
}
