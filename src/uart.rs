use core::fmt;

// レジスタオフセット
const THR: usize = 0; // W: 送信バッファ
const RBR: usize = 0; // R: 受信バッファ
const IER: usize = 1; // R/W: 割り込み許可
const FCR: usize = 2; // W: FIFO 制御
const LCR: usize = 3; // R/W: ラインコントロール
const LSR: usize = 5; // R: ラインステータス

// LCR ビット
const LCR_BAUD_LATCH: u8 = 1 << 7; // DLAB
const LCR_EIGHT_BITS: u8 = 0b11; // 8N1

// FCR ビット
const FCR_FIFO_ENABLE: u8 = 1 << 0;
const FCR_FIFO_CLEAR: u8 = 0b11 << 1;

// LSR ビット
const LSR_RX_READY: u8 = 1 << 0;
const LSR_TX_IDLE: u8 = 1 << 5;

pub struct Uart16550 {
    base: usize,
}

impl Uart16550 {
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    pub fn init(&mut self) {
        unsafe {
            self.write(IER, 0x00); // 割り込み無効
            self.write(LCR, LCR_BAUD_LATCH); // DLAB=1
            self.write(0, 0x03); // DLL: 38400 baud divisor
            self.write(1, 0x00); // DLM
            self.write(LCR, LCR_EIGHT_BITS); // 8N1, DLAB=0
            self.write(FCR, FCR_FIFO_ENABLE | FCR_FIFO_CLEAR);
        }
    }

    pub fn putc(&mut self, c: u8) {
        // THRE が立つまで待ってから送信
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
