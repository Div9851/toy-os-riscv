use core::cell::UnsafeCell;
use core::fmt::{self, Write};

use crate::memlayout::UART0;
use crate::proc;
use crate::spinlock::RawSpinlock;
use crate::uart::Uart16550;

pub struct Console {
    lock: RawSpinlock,
    inner: UnsafeCell<ConsoleInner>,
}

impl Console {
    fn lock(&self) -> &RawSpinlock {
        &self.lock
    }

    fn inner(&self) -> &mut ConsoleInner {
        unsafe { &mut *self.inner.get() }
    }
}

unsafe impl Sync for Console {}

pub struct ConsoleInner {
    uart: Uart16550,
    input: Input,
}

impl ConsoleInner {
    pub fn putc(&mut self, b: u8) {
        self.uart.putc(b);
    }

    pub fn getc(&mut self) -> Option<u8> {
        self.uart.getc()
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.putc(b);
        }
    }

    /// Return true once a committed cooked-input byte can be read.
    fn input_available(&self) -> bool {
        self.input.r != self.input.w
    }

    /// Consume one committed input byte.
    ///
    /// Caller must ensure `input_available` is true.
    fn input_getc(&mut self) -> u8 {
        let c = self.input.buf[self.input.r % INPUT_BUF];
        self.input.r += 1;
        c
    }

    /// Append one received byte to the cooked input buffer.
    ///
    /// Returns true when readers should be woken: a line became available or
    /// the buffer filled. Carriage return is normalized to newline before echo
    /// and storage.
    fn input_putc(&mut self, b: u8) -> bool {
        let b = if b == b'\r' { b'\n' } else { b };

        // Drop input when the cooked buffer is full.
        if self.input.e - self.input.r >= INPUT_BUF {
            return false;
        }

        self.input.buf[self.input.e % INPUT_BUF] = b;
        self.input.e += 1;

        self.putc(b); // echo

        if b == b'\n' || self.input.e - self.input.r == INPUT_BUF {
            self.input.w = self.input.e;
            true
        } else {
            false
        }
    }
}

impl fmt::Write for ConsoleInner {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            self.putc(b);
        }
        Ok(())
    }
}

const INPUT_BUF: usize = 128;

struct Input {
    buf: [u8; INPUT_BUF],
    r: usize,
    w: usize,
    e: usize,
}

impl Input {
    const fn new() -> Self {
        Self {
            buf: [0; 128],
            r: 0,
            w: 0,
            e: 0,
        }
    }
}

pub static CONSOLE: Console = Console {
    lock: RawSpinlock::new(),
    inner: UnsafeCell::new(ConsoleInner {
        uart: Uart16550::new(UART0),
        input: Input::new(),
    }),
};

pub fn init() {
    CONSOLE.lock().acquire();
    CONSOLE.inner().uart.init();
    CONSOLE.lock().release();
}

pub fn _print(args: fmt::Arguments<'_>) {
    CONSOLE.lock().acquire();
    let _ = CONSOLE.inner().write_fmt(args);
    CONSOLE.lock().release();
}

/// Panic-only console output.
///
/// This bypasses the console lock because the panic may have happened while the
/// lock was already held. It writes to the same MMIO UART through a temporary
/// driver instance and intentionally does not touch the lock state.
pub fn _emergency_print(args: fmt::Arguments<'_>) {
    let mut uart = Uart16550::new(UART0);
    let _ = uart.write_fmt(args);
}

pub fn write_bytes(buf: &[u8]) {
    CONSOLE.lock().acquire();
    CONSOLE.inner().write_bytes(buf);
    CONSOLE.lock().release();
}

#[macro_export]
macro_rules! print {
      ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
  }

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! emergency_print {
    ($($arg:tt)*) => ($crate::console::_emergency_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! emergency_println {
    () => ($crate::emergency_print!("\n"));
    ($($arg:tt)*) => ($crate::emergency_print!("{}\n", format_args!($($arg)*)));
}

static INPUT_CHAN: u8 = 0;

/// Stable sleep channel for console input readers.
fn input_chan() -> usize {
    core::ptr::addr_of!(INPUT_CHAN) as usize
}

pub fn intr() {
    let mut do_wakeup = false;

    CONSOLE.lock().acquire();

    while let Some(b) = CONSOLE.inner().getc() {
        if CONSOLE.inner().input_putc(b) {
            do_wakeup = true;
        }
    }

    CONSOLE.lock().release();

    if do_wakeup {
        proc::wakeup(input_chan());
    }
}

pub fn read(dst: &mut [u8]) -> isize {
    if dst.is_empty() {
        return 0;
    }

    CONSOLE.lock().acquire();

    let mut n = 0;

    while n < dst.len() {
        while !CONSOLE.inner().input_available() {
            proc::sleep(input_chan(), CONSOLE.lock());
        }

        let c = CONSOLE.inner().input_getc();

        dst[n] = c;
        n += 1;

        if c == b'\n' {
            break;
        }
    }

    CONSOLE.lock().release();
    n as isize
}
