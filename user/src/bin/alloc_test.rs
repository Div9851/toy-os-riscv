#![no_std]
#![no_main]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use user::{exit, println};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let boxed = Box::new(0x1234_5678usize);
    if *boxed != 0x1234_5678 {
        println!("box check failed");
        exit(1);
    }
    drop(boxed);

    let mut values = Vec::new();
    for i in 0..64usize {
        values.push(i);
    }

    let mut sum = 0usize;
    for v in &values {
        sum += *v;
    }

    if sum != (0..64usize).sum() {
        println!("vec sum check failed");
        exit(1);
    }

    drop(values);

    let reused = Box::new(0xfeed_faceusize);
    if *reused != 0xfeed_face {
        println!("reuse check failed");
        exit(1);
    }

    println!("alloc_test ok");
    exit(0);
}
