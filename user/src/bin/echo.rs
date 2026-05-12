#![no_std]
#![no_main]

use user::{Args, exit, println, write_all};

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: usize, argv: *const *const u8) -> ! {
    let args = Args::new(argc, argv);

    for i in 1..args.len() {
        if i > 1 {
            write_all(1, b" ");
        }
        if let Some(arg) = args.get(i) {
            write_all(1, arg);
        }
    }
    println!();

    exit(0);
}
