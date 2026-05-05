use crate::{
    cpu,
    kalloc::{kalloc_zeroed, kfree},
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
    pub fn flags(self) -> u64 {
        self.0 & 0x3ff
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

pub fn walk_user_perm(pt: &mut PageTable, va: VirtAddr, perm: u64) -> Option<Pte> {
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
    Some(pte)
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

pub fn uvmcreate() -> Option<*mut PageTable> {
    kalloc_zeroed().map(|pa| pa.as_mut_ptr::<PageTable>())
}

pub fn proc_pagetable(trapframe: PhysAddr) -> Option<*mut PageTable> {
    let pt = uvmcreate()?;
    unsafe {
        // trampoline: RX, no U
        if mappages(
            &mut *pt,
            VirtAddr(TRAMPOLINE),
            PGSIZE,
            PhysAddr(trampoline_start()),
            PTE_R | PTE_X,
        )
        .is_err()
        {
            freewalk(pt);
            return None;
        }
        // trapframe: RW, no U, no X
        if mappages(
            &mut *pt,
            VirtAddr(TRAPFRAME),
            PGSIZE,
            trapframe,
            PTE_R | PTE_W,
        )
        .is_err()
        {
            uvmunmap(&mut *pt, VirtAddr(TRAMPOLINE), 1, false);
            freewalk(pt);
            return None;
        }
    }
    Some(pt)
}

pub fn copyin(pt: &mut PageTable, dst: &mut [u8], src_va: VirtAddr) -> Option<()> {
    let mut done = 0;
    while done < dst.len() {
        let va_usize = src_va.as_usize().checked_add(done)?;
        if va_usize >= MAXVA {
            return None;
        }
        let va = VirtAddr(va_usize);
        let va_page = va.page_round_down();
        let off = va.as_usize() - va_page.as_usize();
        let n = core::cmp::min(PGSIZE - off, dst.len() - done);

        let pa_page = walk_user_perm(pt, va_page, PTE_R)?.pa();
        unsafe {
            core::ptr::copy_nonoverlapping(
                pa_page.as_ptr::<u8>().add(off),
                dst.as_mut_ptr().add(done),
                n,
            );
        }
        done += n;
    }

    Some(())
}

pub fn copyout(pt: &mut PageTable, dst_va: VirtAddr, src: &[u8]) -> Option<()> {
    let mut done = 0;

    while done < src.len() {
        let va_usize = dst_va.as_usize().checked_add(done)?;
        if va_usize >= MAXVA {
            return None;
        }
        let va = VirtAddr(va_usize);
        let va_page = va.page_round_down();
        let off = va.as_usize() - va_page.as_usize();
        let n = core::cmp::min(PGSIZE - off, src.len() - done);

        let pa_page = walk_user_perm(pt, va_page, PTE_W)?.pa();

        unsafe {
            core::ptr::copy_nonoverlapping(
                src.as_ptr().add(done),
                pa_page.as_mut_ptr::<u8>().add(off),
                n,
            );
        }

        done += n;
    }

    Some(())
}

pub fn proc_freepagetable(pt: *mut PageTable, sz: usize) {
    unsafe {
        uvmunmap(&mut *pt, VirtAddr(TRAMPOLINE), 1, false);
        uvmunmap(&mut *pt, VirtAddr(TRAPFRAME), 1, false);
    }
    uvmfree(pt, sz);
}

fn uvmunmap(pt: &mut PageTable, va: VirtAddr, npages: usize, do_free: bool) {
    assert!(va.is_page_aligned(), "uvmunmap: va not aligned");
    assert!(npages > 0, "uvmunmap: npages must be positive");

    for i in 0..npages {
        let a = VirtAddr(va.as_usize() + i * PGSIZE);

        let pte_ptr = walk(pt, a, false).expect("uvmunmap: walk");
        let pte = unsafe { &mut *pte_ptr };

        if !pte.is_valid() {
            panic!("uvmunmap: not mapped");
        }
        if !pte.is_leaf() {
            panic!("uvmunmap: not leaf");
        }

        if do_free {
            kfree(pte.pa());
        }
        *pte = Pte(0);
    }
}

fn uvmfree(pt: *mut PageTable, sz: usize) {
    let end = VirtAddr(sz).page_round_up();
    let npages = end.as_usize() / PGSIZE;
    if sz > 0 {
        uvmunmap(unsafe { &mut *pt }, VirtAddr(0), npages, true);
    }
    freewalk(pt);
}

fn freewalk(pt: *mut PageTable) {
    let pagetable = unsafe { &mut *pt };

    for pte in pagetable.0.iter_mut() {
        if pte.is_valid() && !pte.is_leaf() {
            let child = pte.next_pagetable();
            freewalk(child);
            *pte = Pte(0);
        } else if pte.is_leaf() {
            panic!("freewalk: leaf");
        }
    }

    kfree(PhysAddr(pt as usize))
}

pub fn uvmcopy(old: &mut PageTable, new: &mut PageTable, sz: usize) -> Option<()> {
    let end = VirtAddr(sz).page_round_up();
    let npages = end.as_usize() / PGSIZE;

    for i in 0..npages {
        let a = VirtAddr(i * PGSIZE);

        let pte = walk_user_perm(old, a, 0).expect("uvmcopy: pte not found");
        let src_pa = pte.pa();
        let dst_pa = match kalloc_zeroed() {
            Some(pa) => pa,
            None => {
                if i > 0 {
                    uvmunmap(new, VirtAddr(0), i, true);
                }
                return None;
            }
        };

        unsafe {
            core::ptr::copy_nonoverlapping(src_pa.as_ptr::<u8>(), dst_pa.as_mut_ptr(), PGSIZE);
        }
        if mappages(new, a, PGSIZE, dst_pa, pte.flags()).is_err() {
            kfree(dst_pa);
            if i > 0 {
                uvmunmap(new, VirtAddr(0), i, true);
            }
            return None;
        }
    }

    Some(())
}
