use core::fmt;

// 16550 register offsets.
const THR: usize = 0; // W: transmit holding register
const RBR: usize = 0; // R: receive buffer register
const IER: usize = 1; // R/W: interrupt enable register
const FCR: usize = 2; // W: FIFO control register
const LCR: usize = 3; // R/W: line control register
const LSR: usize = 5; // R: line status register

// LCR bits.
const LCR_BAUD_LATCH: u8 = 1 << 7; // DLAB
const LCR_EIGHT_BITS: u8 = 0b11; // 8N1

// FCR bits.
const FCR_FIFO_ENABLE: u8 = 1 << 0;
const FCR_FIFO_CLEAR: u8 = 0b11 << 1;

// LSR bits.
const LSR_RX_READY: u8 = 1 << 0;
const LSR_TX_IDLE: u8 = 1 << 5;

// IER bits.
const IER_RX_ENABLE: u8 = 1 << 0;

pub struct Uart16550 {
    base: usize,
}

impl Uart16550 {
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    pub fn init(&mut self) {
        unsafe {
            self.write(IER, 0x00); // Disable interrupts during setup.
            self.write(LCR, LCR_BAUD_LATCH); // DLAB=1
            self.write(0, 0x03); // DLL: 38400 baud divisor
            self.write(1, 0x00); // DLM
            self.write(LCR, LCR_EIGHT_BITS); // 8N1, DLAB=0
            self.write(FCR, FCR_FIFO_ENABLE | FCR_FIFO_CLEAR);
            self.write(IER, IER_RX_ENABLE);
        }
    }

    pub fn getc(&mut self) -> Option<u8> {
        unsafe {
            if self.read(LSR) & LSR_RX_READY == 0 {
                None
            } else {
                Some(self.read(RBR))
            }
        }
    }

    pub fn putc(&mut self, c: u8) {
        // Poll until the transmit holding register is empty.
        while unsafe { self.read(LSR) } & LSR_TX_IDLE == 0 {}
        unsafe { self.write(THR, c) }
    }

    unsafe fn read(&mut self, off: usize) -> u8 {
        unsafe { core::ptr::read_volatile((self.base + off) as *const u8) }
    }

    unsafe fn write(&mut self, off: usize, v: u8) {
        unsafe { core::ptr::write_volatile((self.base + off) as *mut u8, v) }
    }
}

impl fmt::Write for Uart16550 {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            self.putc(b);
        }
        Ok(())
    }
}
