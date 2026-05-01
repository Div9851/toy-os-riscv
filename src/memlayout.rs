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
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct VirtAddr(pub usize);

pub const KERNBASE: usize = 0x8020_0000;
pub const PHYSTOP: usize = 0x8800_0000; // -m 128M 想定

pub const UART0: usize = 0x1000_0000;
pub const CLINT: usize = 0x0200_0000;
pub const PLIC: usize = 0x0c00_0000;
pub const VIRTIO0: usize = 0x1000_1000;

unsafe extern "C" {
    static __kernel_end: u8; // linker.ld で定義
}

pub fn kernel_end() -> usize {
    (&raw const __kernel_end) as usize
}
