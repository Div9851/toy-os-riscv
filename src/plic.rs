use crate::memlayout::PLIC;
use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

pub const UART_IRQ: u32 = 10;

// hart0 S-mode 固定で計算済みのアドレス
const fn plic_priority(irq: u32) -> *mut u32 {
    (PLIC + 4 * irq as usize) as *mut u32
}

const PLIC_SENABLE: *mut u32 = (PLIC + 0x2080) as *mut u32;
const PLIC_STHRESHOLD: *mut u32 = (PLIC + 0x20_1000) as *mut u32;
const PLIC_SCLAIM: *mut u32 = (PLIC + 0x20_1004) as *mut u32;

pub fn init() {
    unsafe {
        // UART IRQ の優先度を 1
        write_volatile(plic_priority(UART_IRQ), 1);

        // hart 0 S-mode の閾値を 0
        write_volatile(PLIC_STHRESHOLD, 0);

        // hart 0 S-mode の Senable に UART IRQ ビットを立てる
        let v = read_volatile(PLIC_SENABLE);
        write_volatile(PLIC_SENABLE, v | (1 << UART_IRQ));

        // sie.SEIE = 1 (bit 9)
        asm!("csrs sie, {0}", in(reg) 1usize << 9);
    }
}

pub fn handle_external() {
    let Some(irq) = claim() else { return };

    match irq {
        UART_IRQ => uart_rx(),
        _ => panic!("plic: unexpected irq {}", irq),
    }

    complete(irq);
}

fn uart_rx() {
    // UART の RX FIFO に積まれているぶん全て読む
    loop {
        let byte = {
            let mut c = crate::console::CONSOLE.lock();
            c.getc()
        };
        match byte {
            Some(b) => crate::println!("rx: {:#x} {:?}", b, b as char),
            None => break,
        }
    }
}

pub fn claim() -> Option<u32> {
    let irq = unsafe { read_volatile(PLIC_SCLAIM) };
    if irq == 0 { None } else { Some(irq) }
}

pub fn complete(irq: u32) {
    unsafe {
        write_volatile(PLIC_SCLAIM, irq);
    }
}
