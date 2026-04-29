// QEMU virt のメモリマップ定数

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
