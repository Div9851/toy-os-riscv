use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
};

use crate::sbrk;

#[repr(C)]
struct Header {
    size: usize,
    next: *mut Header,
}

const ALIGN: usize = 16;
const HEADER_SIZE: usize = align_up_const(core::mem::size_of::<Header>(), ALIGN);
const MIN_PAYLOAD: usize = ALIGN;
const MIN_BLOCK_SIZE: usize = HEADER_SIZE + MIN_PAYLOAD;

const fn align_up_const(x: usize, align: usize) -> usize {
    (x + align - 1) & !(align - 1)
}

fn align_up(x: usize, align: usize) -> Option<usize> {
    x.checked_add(align - 1).map(|v| v & !(align - 1))
}

pub struct UserAllocator {
    head: UnsafeCell<*mut Header>,
}

impl UserAllocator {
    pub const fn new() -> Self {
        Self {
            head: UnsafeCell::new(core::ptr::null_mut()),
        }
    }
}

unsafe impl Sync for UserAllocator {}

unsafe impl GlobalAlloc for UserAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // This first allocator keeps every block 16-byte aligned. Larger
        // alignment requests are left unsupported for now.
        if layout.align() > ALIGN {
            return core::ptr::null_mut();
        }

        let payload_size = match align_up(layout.size().max(1), ALIGN) {
            Some(v) => v,
            None => return core::ptr::null_mut(),
        };
        let needed = match HEADER_SIZE.checked_add(payload_size) {
            Some(v) => v,
            None => return core::ptr::null_mut(),
        };

        if let Some(ptr) = unsafe { self.alloc_from_freelist(needed) } {
            return ptr;
        }

        if unsafe { self.grow_heap(needed).is_none() } {
            return core::ptr::null_mut();
        }

        unsafe {
            self.alloc_from_freelist(needed)
                .unwrap_or(core::ptr::null_mut())
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let header = unsafe { ptr.sub(HEADER_SIZE) as *mut Header };
        unsafe {
            self.insert_free_block(header);
        }
    }
}

impl UserAllocator {
    unsafe fn alloc_from_freelist(&self, needed: usize) -> Option<*mut u8> {
        let head = unsafe { &mut *self.head.get() };

        let mut prev: *mut Header = core::ptr::null_mut();
        let mut cur = *head;

        while !cur.is_null() {
            let cur_size = unsafe { (*cur).size };

            if cur_size >= needed {
                let remaining = cur_size - needed;

                if remaining >= MIN_BLOCK_SIZE {
                    let rest = unsafe { (cur as *mut u8).add(needed) as *mut Header };

                    unsafe {
                        (*rest).size = remaining;
                        (*rest).next = (*cur).next;

                        (*cur).size = needed;
                    }

                    if prev.is_null() {
                        *head = rest;
                    } else {
                        unsafe {
                            (*prev).next = rest;
                        }
                    }
                } else {
                    if prev.is_null() {
                        unsafe {
                            *head = (*cur).next;
                        }
                    } else {
                        unsafe {
                            (*prev).next = (*cur).next;
                        }
                    }
                }

                unsafe {
                    (*cur).next = core::ptr::null_mut();
                }

                let payload = unsafe { (cur as *mut u8).add(HEADER_SIZE) };
                return Some(payload);
            }

            prev = cur;
            cur = unsafe { (*cur).next };
        }

        None
    }

    unsafe fn grow_heap(&self, needed: usize) -> Option<()> {
        const PGSIZE: usize = 4096;

        let total = align_up(needed, PGSIZE)?;
        if total > isize::MAX as usize {
            return None;
        }

        let base = sbrk(total as isize);
        if base < 0 {
            return None;
        }

        let block = base as usize as *mut Header;

        unsafe {
            (*block).size = total;
            (*block).next = core::ptr::null_mut();
            self.insert_free_block(block);
        }

        Some(())
    }

    unsafe fn insert_free_block(&self, block: *mut Header) {
        let head = unsafe { &mut *self.head.get() };

        let mut prev: *mut Header = core::ptr::null_mut();
        let mut cur = *head;

        while !cur.is_null() && (cur as usize) < (block as usize) {
            prev = cur;
            cur = unsafe { (*cur).next };
        }

        unsafe {
            (*block).next = cur;
        }

        if prev.is_null() {
            *head = block;
        } else {
            unsafe {
                (*prev).next = block;
            }
        }

        let mut merged = block;

        if !prev.is_null() && block_end(prev) == block as usize {
            unsafe {
                (*prev).size += (*block).size;
                (*prev).next = (*block).next;
            }
            merged = prev;
        }

        if !cur.is_null() && block_end(merged) == cur as usize {
            unsafe {
                (*merged).size += (*cur).size;
                (*merged).next = (*cur).next;
            }
        }
    }
}

fn block_end(block: *mut Header) -> usize {
    unsafe { block as usize + (*block).size }
}
