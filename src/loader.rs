use core::ptr::read_unaligned;

use crate::fs::{self, InodeRef};
use crate::memlayout::VirtAddr;
use crate::vm::{PTE_R, PTE_U, PTE_W, PTE_X, mappages, uvmunmap};
use crate::{kalloc::kalloc_zeroed, kalloc::kfree, memlayout::PGSIZE, vm::PageTable};

pub static INIT_ELF: &[u8] =
    include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/init");

pub struct LoadedImage {
    pub entry: usize,
    pub sp: usize,
    pub sz: usize,
}

#[repr(C)]
struct Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
} // 64 bytes

#[repr(C)]
struct Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
} // 56 bytes

const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const ET_EXEC: u16 = 2;
const EM_RISCV: u16 = 243;
const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

/// Load an ELF image into an already-created user page table.
///
/// `pt` is expected to contain only the fixed per-process mappings
/// (`TRAMPOLINE` and `TRAPFRAME`). This function adds user mappings for
/// PT_LOAD segments and one user stack page.
///
/// On success, returns `(entry, sp, sz)`, where `sz` is the user address space
/// size including the stack page. On failure, all user mappings added by this
/// function are removed and freed. The caller remains responsible for freeing
/// the page table itself and its fixed mappings.
pub fn load_elf(pt: &mut PageTable, elf: &[u8]) -> Option<LoadedImage> {
    if core::mem::size_of::<Ehdr>() > elf.len() {
        return None;
    }
    let ehdr: Ehdr = unsafe { read_unaligned(elf.as_ptr() as *const Ehdr) };
    if ehdr.e_ident[..4] != ELF_MAGIC {
        return None;
    }
    if ehdr.e_ident[4] != ELFCLASS64 {
        return None;
    }
    if ehdr.e_ident[5] != ELFDATA2LSB {
        return None;
    }
    if ehdr.e_machine != EM_RISCV {
        return None;
    }
    if ehdr.e_type != ET_EXEC {
        return None;
    }
    if ehdr.e_phentsize as usize != core::mem::size_of::<Phdr>() {
        return None;
    }
    let ph_table_end =
        ehdr.e_phoff as usize + (ehdr.e_phnum as usize) * (ehdr.e_phentsize as usize);
    if ph_table_end > elf.len() {
        return None;
    }
    let mut sz: usize = 0;
    for i in 0..ehdr.e_phnum as usize {
        let phoff = ehdr.e_phoff as usize + i * (ehdr.e_phentsize as usize);
        let phdr: Phdr = unsafe { read_unaligned(elf.as_ptr().add(phoff) as *const Phdr) };
        if phdr.p_type != PT_LOAD {
            continue;
        }
        if load_segment(pt, &phdr, elf).is_none() {
            cleanup_user(pt, sz);
            return None;
        }
        let end = (phdr.p_vaddr + phdr.p_memsz) as usize;
        if end > sz {
            sz = end;
        }
    }
    sz = (sz + PGSIZE - 1) & !(PGSIZE - 1);
    let stack_pa = match kalloc_zeroed() {
        Some(pa) => pa,
        None => {
            cleanup_user(pt, sz);
            return None;
        }
    };
    if mappages(pt, VirtAddr(sz), PGSIZE, stack_pa, PTE_U | PTE_R | PTE_W).is_none() {
        kfree(stack_pa);
        cleanup_user(pt, sz);
        return None;
    }
    let sp = sz + PGSIZE;
    sz += PGSIZE;

    Some(LoadedImage {
        entry: ehdr.e_entry as usize,
        sp,
        sz,
    })
}

pub fn load_elf_from_inode(pt: &mut PageTable, inode: InodeRef) -> Option<LoadedImage> {
    let mut ehdr_buf = [0u8; size_of::<Ehdr>()];
    read_exact_inode(inode, 0, &mut ehdr_buf)?;
    let ehdr = unsafe { read_unaligned(ehdr_buf.as_ptr() as *const Ehdr) };
    if ehdr.e_ident[..4] != ELF_MAGIC {
        return None;
    }
    if ehdr.e_ident[4] != ELFCLASS64 {
        return None;
    }
    if ehdr.e_ident[5] != ELFDATA2LSB {
        return None;
    }
    if ehdr.e_machine != EM_RISCV {
        return None;
    }
    if ehdr.e_type != ET_EXEC {
        return None;
    }
    if ehdr.e_phentsize as usize != core::mem::size_of::<Phdr>() {
        return None;
    }
    let ph_table_size = (ehdr.e_phnum as usize).checked_mul(ehdr.e_phentsize as usize)?;
    let _ph_table_end = (ehdr.e_phoff as usize).checked_add(ph_table_size)?;
    let mut sz: usize = 0;
    for i in 0..ehdr.e_phnum as usize {
        let phoff = ehdr.e_phoff as usize + i * (ehdr.e_phentsize as usize);
        let mut phdr_buf = [0u8; size_of::<Phdr>()];
        read_exact_inode(inode, phoff, &mut phdr_buf)?;
        let phdr: Phdr = unsafe { read_unaligned(phdr_buf.as_ptr() as *const Phdr) };
        if phdr.p_type != PT_LOAD {
            continue;
        }
        if load_segment_from_inode(pt, &phdr, inode).is_none() {
            cleanup_user(pt, sz);
            return None;
        }
        let end = (phdr.p_vaddr + phdr.p_memsz) as usize;
        if end > sz {
            sz = end;
        }
    }
    sz = (sz + PGSIZE - 1) & !(PGSIZE - 1);
    let stack_pa = match kalloc_zeroed() {
        Some(pa) => pa,
        None => {
            cleanup_user(pt, sz);
            return None;
        }
    };
    if mappages(pt, VirtAddr(sz), PGSIZE, stack_pa, PTE_U | PTE_R | PTE_W).is_none() {
        kfree(stack_pa);
        cleanup_user(pt, sz);
        return None;
    }
    let sp = sz + PGSIZE;
    sz += PGSIZE;

    Some(LoadedImage {
        entry: ehdr.e_entry as usize,
        sp,
        sz,
    })
}

fn read_exact_inode(inode: fs::InodeRef, off: usize, dst: &mut [u8]) -> Option<()> {
    let n = fs::readi(inode, off, dst);
    if n == dst.len() as isize {
        Some(())
    } else {
        None
    }
}

/// Remove and free user mappings in `[0, sz)`.
///
/// This assumes the loaded user image is densely mapped from VA 0 up to `sz`.
/// That matches the current user linker layout and stack placement. Sparse ELF
/// layouts would require tracking mapped ranges instead.
fn cleanup_user(pt: &mut PageTable, sz: usize) {
    let end = VirtAddr(sz).page_round_up();
    let npages = end.as_usize() / PGSIZE;
    if npages > 0 {
        uvmunmap(pt, VirtAddr(0), npages, true);
    }
}

/// Load one PT_LOAD segment into `pt`.
///
/// The segment virtual address must be page-aligned, `filesz <= memsz`, and the
/// file-backed range must lie within `elf`. On success, all pages for this
/// segment are mapped. On failure, any pages mapped by this call are unmapped
/// and freed before returning `None`.
///
/// Mappings created by earlier segments are left untouched; `exec` is
/// responsible for cleaning those up.
fn load_segment(pt: &mut PageTable, phdr: &Phdr, elf: &[u8]) -> Option<()> {
    if phdr.p_vaddr as usize % PGSIZE != 0 {
        return None;
    }
    if phdr.p_filesz > phdr.p_memsz {
        return None;
    }
    if (phdr.p_offset + phdr.p_filesz) as usize > elf.len() {
        return None;
    }
    let perm = PTE_U
        | (if phdr.p_flags & PF_R != 0 { PTE_R } else { 0 })
        | (if phdr.p_flags & PF_W != 0 { PTE_W } else { 0 })
        | (if phdr.p_flags & PF_X != 0 { PTE_X } else { 0 });
    let start_va = VirtAddr(phdr.p_vaddr as usize);
    let mut mapped_pages = 0;
    let mut off: u64 = 0;
    while off < phdr.p_memsz {
        let pa = match kalloc_zeroed() {
            Some(pa) => pa,
            None => {
                if mapped_pages > 0 {
                    uvmunmap(pt, start_va, mapped_pages, true);
                }
                return None;
            }
        };
        if off < phdr.p_filesz {
            let n = core::cmp::min(PGSIZE as u64, phdr.p_filesz - off) as usize;
            unsafe {
                core::ptr::copy_nonoverlapping(
                    elf.as_ptr().add((phdr.p_offset + off) as usize),
                    pa.as_mut_ptr::<u8>(),
                    n,
                );
            }
        }
        if mappages(
            pt,
            VirtAddr((phdr.p_vaddr + off) as usize),
            PGSIZE,
            pa,
            perm,
        )
        .is_none()
        {
            kfree(pa);
            if mapped_pages > 0 {
                uvmunmap(pt, start_va, mapped_pages, true);
            }
            return None;
        }
        mapped_pages += 1;
        off += PGSIZE as u64;
    }
    Some(())
}

fn load_segment_from_inode(pt: &mut PageTable, phdr: &Phdr, inode: InodeRef) -> Option<()> {
    if phdr.p_vaddr as usize % PGSIZE != 0 {
        return None;
    }
    if phdr.p_filesz > phdr.p_memsz {
        return None;
    }
    let perm = PTE_U
        | (if phdr.p_flags & PF_R != 0 { PTE_R } else { 0 })
        | (if phdr.p_flags & PF_W != 0 { PTE_W } else { 0 })
        | (if phdr.p_flags & PF_X != 0 { PTE_X } else { 0 });
    let start_va = VirtAddr(phdr.p_vaddr as usize);
    let mut mapped_pages = 0;
    let mut off: u64 = 0;
    while off < phdr.p_memsz {
        let pa = match kalloc_zeroed() {
            Some(pa) => pa,
            None => {
                if mapped_pages > 0 {
                    uvmunmap(pt, start_va, mapped_pages, true);
                }
                return None;
            }
        };
        if off < phdr.p_filesz {
            let n = core::cmp::min(PGSIZE as u64, phdr.p_filesz - off) as usize;
            let dst = unsafe { core::slice::from_raw_parts_mut(pa.as_mut_ptr::<u8>(), n) };
            if read_exact_inode(inode, (phdr.p_offset + off) as usize, dst).is_none() {
                kfree(pa);
                if mapped_pages > 0 {
                    uvmunmap(pt, start_va, mapped_pages, true);
                }
                return None;
            }
        }
        if mappages(
            pt,
            VirtAddr((phdr.p_vaddr + off) as usize),
            PGSIZE,
            pa,
            perm,
        )
        .is_none()
        {
            kfree(pa);
            if mapped_pages > 0 {
                uvmunmap(pt, start_va, mapped_pages, true);
            }
            return None;
        }
        mapped_pages += 1;
        off += PGSIZE as u64;
    }
    Some(())
}
