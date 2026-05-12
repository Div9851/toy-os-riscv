use crate::{proc, spinlock::RawSpinlock};

const NPIPE: usize = 64;
const PIPESIZE: usize = 512;

pub struct Pipe {
    used: bool,
    lock: RawSpinlock,
    data: [u8; PIPESIZE],
    nread: usize,
    nwrite: usize,
    readopen: bool,
    writeopen: bool,
}

impl Pipe {
    const fn unused() -> Self {
        Self {
            used: false,
            lock: RawSpinlock::new(),
            data: [0; PIPESIZE],
            nread: 0,
            nwrite: 0,
            readopen: false,
            writeopen: false,
        }
    }
}

static PIPE_TABLE_LOCK: RawSpinlock = RawSpinlock::new();
static mut PIPES: [Pipe; NPIPE] = [const { Pipe::unused() }; NPIPE];

fn read_chan(p: *const Pipe) -> usize {
    p as usize
}

fn write_chan(p: *const Pipe) -> usize {
    p as usize + 1
}

pub fn alloc() -> Option<*mut Pipe> {
    PIPE_TABLE_LOCK.acquire();

    let base = core::ptr::addr_of_mut!(PIPES) as *mut Pipe;
    for i in 0..NPIPE {
        let pipe = unsafe { &mut *base.add(i) };
        if !pipe.used {
            pipe.used = true;
            pipe.nread = 0;
            pipe.nwrite = 0;
            pipe.readopen = true;
            pipe.writeopen = true;

            let ptr = pipe as *mut Pipe;
            PIPE_TABLE_LOCK.release();
            return Some(ptr);
        }
    }

    PIPE_TABLE_LOCK.release();
    None
}

pub fn free_allocated(p: *mut Pipe) {
    let pipe = unsafe { &mut *p };

    pipe.lock.acquire();
    pipe.readopen = false;
    pipe.writeopen = false;
    pipe.nread = 0;
    pipe.nwrite = 0;
    pipe.lock.release();

    PIPE_TABLE_LOCK.acquire();
    pipe.used = false;
    PIPE_TABLE_LOCK.release();
}

pub fn close(p: *mut Pipe, writable: bool) {
    let pipe = unsafe { &mut *p };

    pipe.lock.acquire();

    if writable {
        pipe.writeopen = false;
        proc::wakeup(read_chan(p));
    } else {
        pipe.readopen = false;
        proc::wakeup(write_chan(p));
    }

    let do_free = !pipe.readopen && !pipe.writeopen;

    pipe.lock.release();

    if do_free {
        PIPE_TABLE_LOCK.acquire();
        pipe.used = false;
        PIPE_TABLE_LOCK.release();
    }
}

pub fn read(p: *mut Pipe, dst: &mut [u8]) -> isize {
    let pipe = unsafe { &mut *p };

    pipe.lock.acquire();

    while pipe.nread == pipe.nwrite && pipe.writeopen {
        proc::sleep(read_chan(p), &pipe.lock);
    }

    let mut n = 0;
    while n < dst.len() && pipe.nread != pipe.nwrite {
        dst[n] = pipe.data[pipe.nread % PIPESIZE];
        pipe.nread += 1;
        n += 1;
    }

    proc::wakeup(write_chan(p));
    pipe.lock.release();

    n as isize
}

pub fn write(p: *mut Pipe, src: &[u8]) -> isize {
    let pipe = unsafe { &mut *p };

    pipe.lock.acquire();

    let mut n = 0;
    while n < src.len() {
        while pipe.nwrite == pipe.nread + PIPESIZE {
            if !pipe.readopen {
                pipe.lock.release();
                return -1;
            }
            proc::wakeup(read_chan(p));
            proc::sleep(write_chan(p), &pipe.lock);
        }

        if !pipe.readopen {
            pipe.lock.release();
            return -1;
        }

        pipe.data[pipe.nwrite % PIPESIZE] = src[n];
        pipe.nwrite += 1;
        n += 1;
    }

    proc::wakeup(read_chan(p));
    pipe.lock.release();

    n as isize
}
