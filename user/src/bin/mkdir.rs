#![no_std]
#![no_main]

use user::{Args, exit, mkdir, println, write_all};

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: usize, argv: *const *const u8) -> ! {
    let args = Args::new(argc, argv);

    if args.len() < 2 {
        println!("usage: mkdir dir...");
        exit(1);
    }

    let mut failed = false;
    for i in 1..args.len() {
        let path = args.get(i).unwrap();
        if mkdir(path) < 0 {
            write_all(1, path);
            println!(": mkdir failed");
            failed = true;
        }
    }

    exit(if failed { 1 } else { 0 });
}
