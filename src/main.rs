#![no_std]
#![no_main]

use core::arch::global_asm;
use core::panic::PanicInfo;

global_asm!(
    r#"
.section .text.entry
.global _start
_start:
    la sp, __stack_top

    la t0, __bss_start
    la t1, __bss_end
1:
    bgeu t0, t1, 2f
    sd zero, 0(t0)
    addi t0, t0, 8
    j 1b
2:
    tail kmain
"#
);

fn sbi_console_putchar(c: u8) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") c as usize,
            in("a7") 1usize,
            lateout("a0") _,
        )
    }
}

fn sbi_console_print(s: &str) {
    for &b in s.as_bytes() {
        sbi_console_putchar(b);
    }
}

struct SbiConsole;

impl core::fmt::Write for SbiConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        sbi_console_print(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::SbiConsole, $($arg)*);
    }};
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _= writeln!($crate::SbiConsole, $($arg)*);
    }};
}

#[unsafe(no_mangle)]
extern "C" fn kmain(hartid: usize, dtb: usize) -> ! {
    panic!("panic test");
    // println!("Hello, world!");
    // println!("hartid = {}, dtb = {:#x}", hartid, dtb);
    // loop {
    //     core::hint::spin_loop();
    // }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n!!! KERNEL PANIC: {}", info);
    loop {
        core::hint::spin_loop();
    }
}
