use crate::{
    cpu,
    kalloc::kalloc_zeroed,
    memlayout::{
        CLINT, KERNBASE, MAXVA, PGSIZE, PHYSTOP, PLIC, PhysAddr, TRAMPOLINE, TRAPFRAME, UART0,
        VirtAddr, erodata, etext, trampoline_start,
    },
};

pub const PTE_V: u64 = 1 << 0;
pub const PTE_R: u64 = 1 << 1;
pub const PTE_W: u64 = 1 << 2;
pub const PTE_X: u64 = 1 << 3;
pub const PTE_U: u64 = 1 << 4;
pub const PTE_G: u64 = 1 << 5;
pub const PTE_A: u64 = 1 << 6;
pub const PTE_D: u64 = 1 << 7;

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Pte(pub u64);

impl Pte {
    pub fn new_leaf(pa: PhysAddr, flags: u64) -> Self {
        assert!(
            pa.is_page_aligned(),
            "new_leaf: pa not aligned: {:#x}",
            pa.as_usize()
        );
        Self((pa.ppn() << 10) | flags | PTE_V | PTE_A | PTE_D)
    }
    pub fn new_table(pa: PhysAddr) -> Self {
        assert!(
            pa.is_page_aligned(),
            "new_table: pa not aligned: {:#x}",
            pa.as_usize()
        );
        Self((pa.ppn() << 10) | PTE_V)
    }
    pub fn is_valid(self) -> bool {
        self.0 & PTE_V != 0
    }
    pub fn is_leaf(self) -> bool {
        self.is_valid() && (self.0 & (PTE_R | PTE_W | PTE_X)) != 0
    }
    pub fn pa(self) -> PhysAddr {
        let ppn = (self.0 >> 10) & ((1u64 << 44) - 1);
        PhysAddr((ppn << 12) as usize)
    }
    pub fn next_pagetable(self) -> *mut PageTable {
        self.pa().as_mut_ptr::<PageTable>()
    }
}

#[repr(C, align(4096))]
pub struct PageTable(pub [Pte; 512]);

const _: () = assert!(core::mem::size_of::<PageTable>() == 4096);

pub fn walk(pt: &mut PageTable, va: VirtAddr, alloc: bool) -> Option<*mut Pte> {
    let mut pt: *mut PageTable = pt;
    for level in [2, 1] {
        let idx = (va.0 >> (12 + 9 * level)) & 0x1ff;
        let pte = unsafe { &mut (*pt).0[idx] };
        if pte.is_valid() {
            if pte.is_leaf() {
                return None;
            }
            pt = pte.next_pagetable();
        } else if alloc {
            let pa = kalloc_zeroed()?;
            *pte = Pte::new_table(pa);
            pt = pa.as_mut_ptr::<PageTable>();
        } else {
            return None;
        }
    }
    let idx = (va.0 >> 12) & 0x1ff;
    Some(unsafe { &mut (*pt).0[idx] })
}

pub fn walk_user_perm(pt: &mut PageTable, va: VirtAddr, perm: u64) -> Option<PhysAddr> {
    if va.as_usize() >= MAXVA {
        return None;
    }
    let pte = unsafe { *walk(pt, va, false)? };
    if !pte.is_valid() {
        return None;
    }
    if pte.0 & PTE_U == 0 {
        return None;
    }
    if pte.0 & perm != perm {
        return None;
    }
    if !pte.is_leaf() {
        return None;
    }
    Some(pte.pa())
}

pub fn mappages(
    pt: &mut PageTable,
    va: VirtAddr,
    size: usize,
    pa: PhysAddr,
    flags: u64,
) -> Result<(), &'static str> {
    assert!(
        va.is_page_aligned(),
        "mappages: va not aligned: {:#x}",
        va.as_usize()
    );
    assert!(
        pa.is_page_aligned(),
        "mappages: pa not aligned: {:#x}",
        pa.as_usize()
    );
    assert!(size > 0);
    let mut va = va;
    let last = VirtAddr(va.0 + size).page_round_up();
    let mut pa = pa;

    while va < last {
        let pte_ptr = walk(pt, va, true).ok_or("walk: no mem")?;
        let pte = unsafe { &mut *pte_ptr };
        if pte.is_valid() {
            return Err("mappages: remap");
        };
        *pte = Pte::new_leaf(pa, flags);
        va = VirtAddr(va.0 + PGSIZE);
        pa = PhysAddr(pa.0 + PGSIZE);
    }

    Ok(())
}

pub fn kvmmake() -> &'static mut PageTable {
    let pa = kalloc_zeroed().expect("kvmmake: out of memory");
    let pt = unsafe { &mut *pa.as_mut_ptr::<PageTable>() };

    // MMIO
    kvmmap(pt, UART0, PGSIZE, PTE_R | PTE_W);
    kvmmap(pt, CLINT, 0x10000, PTE_R | PTE_W);
    kvmmap(pt, PLIC, 0x40_0000, PTE_R | PTE_W);

    // text RX
    kvmmap_range(pt, KERNBASE, etext(), PTE_R | PTE_X);
    // rodata R
    kvmmap_range(pt, etext(), erodata(), PTE_R);
    // data + bss + stack + free pages: RW
    kvmmap_range(pt, erodata(), PHYSTOP, PTE_R | PTE_W);

    mappages(
        pt,
        VirtAddr(TRAMPOLINE),
        PGSIZE,
        PhysAddr(trampoline_start()),
        PTE_R | PTE_X,
    )
    .unwrap();

    pt
}

fn kvmmap(pt: &mut PageTable, va_pa: usize, size: usize, flags: u64) {
    mappages(pt, VirtAddr(va_pa), size, PhysAddr(va_pa), flags).unwrap();
}

fn kvmmap_range(pt: &mut PageTable, start: usize, end: usize, flags: u64) {
    kvmmap(pt, start, end - start, flags);
}

const SATP_MODE_SV39: u64 = 8;

pub fn make_satp(root: *const PageTable) -> u64 {
    let pa = root as u64;
    (SATP_MODE_SV39 << 60) | (pa >> 12)
}

pub fn kvminithart(pt: &PageTable) {
    unsafe {
        cpu::sfence_vma();
        cpu::w_satp(make_satp(pt));
        cpu::sfence_vma();
    }
}

pub fn uvmcreate() -> *mut PageTable {
    let pa = kalloc_zeroed().expect("uvmcreate: out of memory");
    pa.as_mut_ptr::<PageTable>()
}

pub fn proc_pagetable(trapframe: PhysAddr) -> *mut PageTable {
    let pt = uvmcreate();
    unsafe {
        // trampoline: RX, no U
        mappages(
            &mut *pt,
            VirtAddr(TRAMPOLINE),
            PGSIZE,
            PhysAddr(trampoline_start()),
            PTE_R | PTE_X,
        )
        .unwrap();
        // trapframe: RW, no U, no X
        mappages(
            &mut *pt,
            VirtAddr(TRAPFRAME),
            PGSIZE,
            trapframe,
            PTE_R | PTE_W,
        )
        .unwrap();
    }
    pt
}

pub enum CopyError {
    Fault, // 不正な VA / unmapped
}

pub fn copyin(pt: *mut PageTable, dst: &mut [u8], src_va: VirtAddr) -> Result<(), CopyError> {
    let mut done = 0;
    while done < dst.len() {
        let va_usize = src_va
            .as_usize()
            .checked_add(done)
            .ok_or(CopyError::Fault)?;
        if va_usize >= MAXVA {
            return Err(CopyError::Fault);
        }
        let va = VirtAddr(va_usize);
        let va_page = va.page_round_down();
        let off = va.as_usize() - va_page.as_usize();
        let n = core::cmp::min(PGSIZE - off, dst.len() - done);

        let pa_page =
            walk_user_perm(unsafe { &mut *pt }, va_page, PTE_R).ok_or(CopyError::Fault)?;
        unsafe {
            core::ptr::copy_nonoverlapping(
                pa_page.as_ptr::<u8>().add(off),
                dst.as_mut_ptr().add(done),
                n,
            );
        }
        done += n;
    }

    Ok(())
}
