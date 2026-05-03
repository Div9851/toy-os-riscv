use core::fmt::{self, Write};

use crate::memlayout::UART0;
use crate::spinlock::Spinlock;
use crate::uart::Uart16550;

pub static CONSOLE: Spinlock<Uart16550> = Spinlock::new(Uart16550::new(UART0));

pub fn init() {
    CONSOLE.lock().init();
}

pub fn _print(args: fmt::Arguments<'_>) {
    let _ = CONSOLE.lock().write_fmt(args);
}

/// panic 専用: Mutex を経由せず、ローカルに UART を作って書く。
/// ロック保有中に panic しても deadlock しない。
pub fn _emergency_print(args: fmt::Arguments<'_>) {
    let mut uart = Uart16550::new(UART0);
    let _ = uart.write_fmt(args);
}

pub fn write_bytes(buf: &[u8]) {
    let mut c = CONSOLE.lock();
    for &b in buf {
        c.putc(b);
    }
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
