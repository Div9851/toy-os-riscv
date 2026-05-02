// QEMU virt のメモリマップ定数

pub const PGSIZE: usize = 4096;
pub const PGSHIFT: u32 = 12;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct PhysAddr(pub usize);

impl PhysAddr {
    pub fn as_usize(self) -> usize {
        self.0
    }

    pub fn is_page_aligned(self) -> bool {
        self.0 & (PGSIZE - 1) == 0
    }

    pub fn page_round_down(self) -> Self {
        Self(self.0 & !(PGSIZE - 1))
    }

    pub fn page_round_up(self) -> Self {
        Self((self.0 + PGSIZE - 1) & !(PGSIZE - 1))
    }

    pub fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    pub fn ppn(self) -> u64 {
        (self.0 as u64) >> PGSHIFT // 44 bit
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct VirtAddr(pub usize);

impl VirtAddr {
    pub fn as_usize(self) -> usize {
        self.0
    }

    pub fn is_page_aligned(self) -> bool {
        self.0 & (PGSIZE - 1) == 0
    }

    pub fn page_round_down(self) -> Self {
        Self(self.0 & !(PGSIZE - 1))
    }

    pub fn page_round_up(self) -> Self {
        Self((self.0 + PGSIZE - 1) & !(PGSIZE - 1))
    }
}

pub const KERNBASE: usize = 0x8020_0000;
pub const PHYSTOP: usize = 0x8800_0000; // -m 128M 想定

pub const UART0: usize = 0x1000_0000;
pub const CLINT: usize = 0x0200_0000;
pub const PLIC: usize = 0x0c00_0000;
pub const VIRTIO0: usize = 0x1000_1000;

pub const MAXVA: usize = 1 << 38; // 0x40_0000_0000
pub const TRAMPOLINE: usize = MAXVA - PGSIZE;
pub const TRAPFRAME: usize = MAXVA - 2 * PGSIZE;

unsafe extern "C" {
    // linker.ld で定義
    static __trampoline_start: u8;
    static __etext: u8;
    static __erodata: u8;
    static __kernel_end: u8;

    static uservec: u8;
    static userret: u8;
}

pub fn trampoline_start() -> usize {
    (&raw const __trampoline_start) as usize
}

pub fn etext() -> usize {
    (&raw const __etext) as usize
}

pub fn erodata() -> usize {
    (&raw const __erodata) as usize
}

pub fn kernel_end() -> usize {
    (&raw const __kernel_end) as usize
}

// user pagetable にはトランポリンページの pa はマッピングされていない
// TRAMPOLINE を元に va を算出する必要がある
pub fn trampoline_uservec_va() -> usize {
    TRAMPOLINE + (core::ptr::addr_of!(uservec) as usize - trampoline_start())
}

pub fn trampoline_userret_va() -> usize {
    TRAMPOLINE + (core::ptr::addr_of!(userret) as usize - trampoline_start())
}
