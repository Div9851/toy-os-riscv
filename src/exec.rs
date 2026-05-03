use core::ptr::read_unaligned;

use crate::memlayout::VirtAddr;
use crate::vm::{PTE_R, PTE_U, PTE_W, PTE_X, mappages};
use crate::{kalloc::kalloc_zeroed, memlayout::PGSIZE, vm::PageTable};

pub static INIT_ELF: &[u8] =
    include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/init");

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

pub fn exec(
    pt: *mut PageTable,
    elf: &[u8],
) -> Result<
    (
        usize, /* entry */
        usize, /* sp */
        usize, /* sz */
    ),
    &'static str,
> {
    if core::mem::size_of::<Ehdr>() > elf.len() {
        return Err("exec: too short ELF file");
    }
    let ehdr: Ehdr = unsafe { core::ptr::read_unaligned(elf.as_ptr() as *const Ehdr) };
    if ehdr.e_ident[..4] != ELF_MAGIC {
        return Err("exec: bad e_ident magic");
    }
    if ehdr.e_ident[4] != ELFCLASS64 {
        return Err("exec: bad e_ident[EI_CLASS] (expected ELFCLASS64)");
    }
    if ehdr.e_ident[5] != ELFDATA2LSB {
        return Err("exec: bad e_ident[EI_DATA] (expected ELFDATA2LSB)");
    }
    if ehdr.e_machine != EM_RISCV {
        return Err("exec: bad e_machine (expected EM_RISCV)");
    }
    if ehdr.e_type != ET_EXEC {
        return Err("exec: bad e_type (expected ET_EXEC)");
    }
    if ehdr.e_phentsize as usize != core::mem::size_of::<Phdr>() {
        return Err("exec: bad e_phentsize");
    }
    let ph_table_end =
        ehdr.e_phoff as usize + (ehdr.e_phnum as usize) * (ehdr.e_phentsize as usize);
    if ph_table_end > elf.len() {
        return Err("exec: phdr table out of bounds");
    }
    let mut sz: usize = 0;
    for i in 0..ehdr.e_phnum as usize {
        let phoff = ehdr.e_phoff as usize + i * (ehdr.e_phentsize as usize);
        let phdr: Phdr = unsafe { read_unaligned(elf.as_ptr().add(phoff) as *const Phdr) };
        if phdr.p_type != PT_LOAD {
            continue;
        }
        load_segment(unsafe { &mut *pt }, &phdr, elf)?;
        let end = (phdr.p_vaddr + phdr.p_memsz) as usize;
        if end > sz {
            sz = end;
        }
    }
    sz = (sz + PGSIZE - 1) & !(PGSIZE - 1);
    let stack_pa = kalloc_zeroed().ok_or("exec: stack alloc")?;
    mappages(
        unsafe { &mut *pt },
        VirtAddr(sz),
        PGSIZE,
        stack_pa,
        PTE_U | PTE_R | PTE_W,
    )?;
    let sp = sz + PGSIZE;
    sz += PGSIZE;

    Ok((ehdr.e_entry as usize, sp, sz))
}

fn load_segment(pt: &mut PageTable, phdr: &Phdr, elf: &[u8]) -> Result<(), &'static str> {
    assert!(
        phdr.p_vaddr as usize % PGSIZE == 0,
        "exec: p_vaddr not page-aligned"
    );
    assert!(
        phdr.p_filesz <= phdr.p_memsz,
        "load_segment: p_filesz > p_memsz"
    );
    if (phdr.p_offset + phdr.p_filesz) as usize > elf.len() {
        return Err("exec: PT_LOAD file range out of bounds");
    }
    let perm = PTE_U
        | (if phdr.p_flags & PF_R != 0 { PTE_R } else { 0 })
        | (if phdr.p_flags & PF_W != 0 { PTE_W } else { 0 })
        | (if phdr.p_flags & PF_X != 0 { PTE_X } else { 0 });
    let mut off: u64 = 0;
    while off < phdr.p_memsz {
        let pa = kalloc_zeroed().ok_or("load_segment: out of memory")?;
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
        mappages(
            pt,
            VirtAddr((phdr.p_vaddr + off) as usize),
            PGSIZE,
            pa,
            perm,
        )?;
        off += PGSIZE as u64;
    }
    Ok(())
}
