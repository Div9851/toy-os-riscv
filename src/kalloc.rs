use crate::memlayout::{PGSIZE, PHYSTOP, PhysAddr, kernel_end};
use crate::spinlock::Spinlock;
use core::ptr;

#[repr(C)]
struct Run {
    next: *mut Run,
}

struct KMem {
    head: *mut Run,
}

unsafe impl Send for KMem {}

static KMEM: Spinlock<KMem> = Spinlock::new(KMem {
    head: ptr::null_mut(),
});

/// Initialize the physical page allocator with pages after the kernel image.
pub fn init() {
    let start = PhysAddr(kernel_end());
    let end = PhysAddr(PHYSTOP);

    assert!(start.is_page_aligned(), "__kernel_end must be page-aligned");

    freerange(start, end);
}

/// Free every complete page in `[start, end)`.
///
/// `start` is expected to be page-aligned by the linker. A non-aligned `end` is
/// rounded down naturally by the loop condition.
fn freerange(start: PhysAddr, end: PhysAddr) {
    let mut p = start.as_usize();
    while p + PGSIZE <= end.as_usize() {
        kfree(PhysAddr(p));
        p += PGSIZE;
    }
}

/// Return one physical page to the allocator.
///
/// The page must be page-aligned, within the managed RAM range, and not
/// currently allocated elsewhere. The page contents are overwritten with junk
/// before publishing it to the freelist.
pub fn kfree(pa: PhysAddr) {
    assert!(
        pa.is_page_aligned(),
        "kfree: pa not page-aligned: {:#x}",
        pa.as_usize()
    );
    assert!(
        pa >= PhysAddr(kernel_end()),
        "kfree: pa below kernel end: {:#x}",
        pa.as_usize()
    );
    assert!(
        pa < PhysAddr(PHYSTOP),
        "kfree: pa beyond PHYSTOP: {:#x}",
        pa.as_usize()
    );

    // junk fill
    unsafe {
        ptr::write_bytes(pa.as_mut_ptr::<u8>(), 0x05, PGSIZE);
    }

    let r = pa.as_mut_ptr::<Run>();
    let mut kmem = KMEM.lock();
    unsafe {
        (*r).next = kmem.head;
    }
    kmem.head = r;
}

/// Allocate one physical page.
///
/// The returned page contains unspecified junk bytes. Use `kalloc_zeroed` when
/// the caller needs a zero-filled page table, trapframe, or user page.
pub fn kalloc() -> Option<PhysAddr> {
    let mut kmem = KMEM.lock();
    let r = kmem.head;
    if r.is_null() {
        return None;
    }
    kmem.head = unsafe { (*r).next };
    Some(PhysAddr(r as usize))
}

/// Allocate one physical page and zero-fill it before returning.
pub fn kalloc_zeroed() -> Option<PhysAddr> {
    let pa = kalloc()?;
    unsafe {
        ptr::write_bytes(pa.as_mut_ptr::<u8>(), 0, PGSIZE);
    }
    Some(pa)
}
