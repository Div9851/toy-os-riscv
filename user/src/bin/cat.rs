#![no_std]
#![no_main]

use user::{Args, O_RDONLY, close, exit, open, println, read, write_all};

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: usize, argv: *const *const u8) -> ! {
    let args = Args::new(argc, argv);
    if argc != 2 {
        exit(1);
    }
    let fd = open(args.get(1).unwrap(), O_RDONLY) as i32;
    if fd < 0 {
        println!("open failed");
        exit(1);
    }
    let mut buf = [0u8; 64];
    loop {
        let n = read(fd, &mut buf);
        if n > 0 {
            write_all(1, &buf[..n as usize]);
        } else if n == 0 {
            close(fd);
            exit(0);
        } else {
            println!("read failed");
            close(fd);
            exit(1);
        }
    }
}
