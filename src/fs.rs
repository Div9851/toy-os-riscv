use crate::spinlock::{RawSpinlock, Spinlock, SpinlockGuard};

const BSIZE: usize = 1024;
const FSSIZE: usize = 1000;
const NINODES: usize = 200; // on-disk inode count
const NINODE: usize = 50; // in-memory inode slot
const NLOG: usize = 0;

const ROOTINO: u32 = 1;
const FSMAGIC: u32 = 0x1020_3040;

const NDIRECT: usize = 12;
const NINDIRECT: usize = BSIZE / core::mem::size_of::<u32>();
const MAXFILE: usize = NDIRECT + NINDIRECT;

const IPB: usize = BSIZE / core::mem::size_of::<Dinode>(); // Inodes Per Block
const BPB: usize = BSIZE * 8; // Bits Per Block

const NINODEBLOCKS: usize = NINODES.div_ceil(IPB);
const NBITMAP: usize = FSSIZE.div_ceil(BPB);
const NMETA: usize = 2 + NLOG + NINODEBLOCKS + NBITMAP;

const LOGSTART: usize = 2;
const INODESTART: usize = LOGSTART + NLOG;
const BMAPSTART: usize = INODESTART + NINODEBLOCKS;
const DATASTART: usize = BMAPSTART + NBITMAP;
const NBLOCKS: usize = FSSIZE - NMETA;

const T_DIR: i16 = 1;
const T_FILE: i16 = 2;
const T_DEVICE: i16 = 3;

static mut DISK: [[u8; BSIZE]; FSSIZE] = [[0; BSIZE]; FSSIZE];

fn bread(blockno: u32, dst: &mut [u8; BSIZE]) {
    unsafe {
        dst.copy_from_slice(&DISK[blockno as usize]);
    }
}

fn bwrite(blockno: u32, src: &[u8; BSIZE]) {
    unsafe {
        DISK[blockno as usize].copy_from_slice(src);
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SuperBlock {
    magic: u32,
    size: u32,
    nblocks: u32,
    ninodes: u32,
    nlog: u32,
    logstart: u32,
    inodestart: u32,
    bmapstart: u32,
}

static SB: SuperBlock = SuperBlock {
    magic: FSMAGIC,
    size: FSSIZE as u32,
    nblocks: NBLOCKS as u32,
    ninodes: NINODES as u32,
    nlog: NLOG as u32,
    logstart: LOGSTART as u32,
    inodestart: INODESTART as u32,
    bmapstart: BMAPSTART as u32,
};
#[repr(C)]
#[derive(Clone, Copy)]
struct Dinode {
    typ: i16,
    major: i16,
    minor: i16,
    nlink: i16,
    size: u32,
    addrs: [u32; NDIRECT + 1],
}

fn iblock(inum: u32) -> u32 {
    inum / IPB as u32 + SB.inodestart
}

fn read_dinode(inum: u32) -> Dinode {
    let blockno = iblock(inum);
    let idx = (inum as usize) % IPB;
    let off = idx * core::mem::size_of::<Dinode>();

    let mut buf = [0u8; BSIZE];
    bread(blockno, &mut buf);
    unsafe { core::ptr::read_unaligned(buf.as_ptr().add(off) as *const Dinode) }
}

fn write_dinode(inum: u32, dip: &Dinode) {
    let blockno = iblock(inum);
    let idx = (inum as usize) % IPB;
    let off = idx * core::mem::size_of::<Dinode>();

    let mut buf = [0u8; BSIZE];
    bread(blockno, &mut buf);
    unsafe {
        core::ptr::write_unaligned(buf.as_mut_ptr().add(off) as *mut Dinode, *dip);
    }
    bwrite(iblock(inum), &buf);
}

fn bblock(blockno: u32) -> u32 {
    blockno / BPB as u32 + SB.bmapstart
}

fn balloc() -> Option<u32> {
    let mut buf = [0u8; BSIZE];
    for blockno in DATASTART..FSSIZE {
        let bitmap_blockno = bblock(blockno as u32);
        let bi = blockno % BPB;
        let byte = bi / 8;
        let mask = 1u8 << (bi % 8);

        bread(bitmap_blockno, &mut buf);

        if buf[byte] & mask == 0 {
            buf[byte] |= mask;
            bwrite(bitmap_blockno, &buf);
            bwrite(blockno as u32, &[0u8; BSIZE]);
            return Some(blockno as u32);
        }
    }
    None
}

fn bfree(blockno: u32) {
    if blockno < DATASTART as u32 || blockno >= FSSIZE as u32 {
        panic!("bfree: invalid block");
    }

    let bitmap_blockno = bblock(blockno);
    let bi = (blockno as usize) % BPB;
    let byte = bi / 8;
    let mask = 1u8 << (bi % 8);

    let mut buf = [0u8; BSIZE];
    bread(bitmap_blockno, &mut buf);
    if buf[byte] & mask == 0 {
        panic!("freeing free block");
    }
    buf[byte] &= !mask;
    bwrite(bitmap_blockno, &buf);
}

pub struct Inode {
    inum: u32,
    refcnt: usize,
    valid: bool,

    typ: i16,
    major: i16,
    minor: i16,
    nlink: i16,
    size: u32,
    addrs: [u32; NDIRECT + 1],
}

pub type InodeRef = &'static Spinlock<Inode>;

impl Inode {
    const fn empty() -> Self {
        Self {
            inum: 0,
            refcnt: 0,
            valid: false,
            typ: 0,
            major: 0,
            minor: 0,
            nlink: 0,
            size: 0,
            addrs: [0; NDIRECT + 1],
        }
    }
}

static ICACHE_LOCK: RawSpinlock = RawSpinlock::new();

static ICACHE: [Spinlock<Inode>; NINODE] = [const { Spinlock::new(Inode::empty()) }; NINODE];

fn iget(inum: u32) -> Option<&'static Spinlock<Inode>> {
    ICACHE_LOCK.acquire();

    for slot in ICACHE.iter() {
        let mut ip = slot.lock();
        if ip.refcnt > 0 && ip.inum == inum {
            ip.refcnt += 1;
            drop(ip);
            ICACHE_LOCK.release();
            return Some(slot);
        }
    }

    for slot in ICACHE.iter() {
        let mut ip = slot.lock();
        if ip.refcnt == 0 {
            ip.inum = inum;
            ip.refcnt = 1;
            ip.valid = false;
            drop(ip);
            ICACHE_LOCK.release();
            return Some(slot);
        }
    }

    ICACHE_LOCK.release();
    None
}

fn ilock(ip: InodeRef) -> SpinlockGuard<'static, Inode> {
    let mut ip = ip.lock();

    if !ip.valid {
        let dip = read_dinode(ip.inum);
        ip.typ = dip.typ;
        ip.major = dip.major;
        ip.minor = dip.minor;
        ip.nlink = dip.nlink;
        ip.size = dip.size;
        ip.addrs = dip.addrs;
        ip.valid = true;
    }

    ip
}

pub fn idup(ip: InodeRef) -> InodeRef {
    ICACHE_LOCK.acquire();

    {
        let mut guard = ip.lock();
        if guard.refcnt < 1 {
            panic!("idup: no ref");
        }
        guard.refcnt += 1;
    }

    ICACHE_LOCK.release();
    ip
}

pub fn iput(ip: InodeRef) {
    ICACHE_LOCK.acquire();

    {
        let mut guard = ip.lock();
        if guard.refcnt < 1 {
            panic!("iput: no ref");
        }
        guard.refcnt -= 1;
    }

    ICACHE_LOCK.release();
}

fn bmap_lookup(ip: &Inode, bn: u32) -> Option<u32> {
    if bn < NDIRECT as u32 {
        let blockno = ip.addrs[bn as usize];
        return (blockno != 0).then_some(blockno);
    }

    let bn = bn - NDIRECT as u32;

    if bn < NINDIRECT as u32 {
        let indirect_block = ip.addrs[NDIRECT];
        if indirect_block == 0 {
            return None;
        }

        let mut buf = [0u8; BSIZE];
        bread(indirect_block, &mut buf);

        let off = bn as usize * core::mem::size_of::<u32>();
        let blockno = unsafe { core::ptr::read_unaligned(buf.as_ptr().add(off) as *const u32) };
        return (blockno != 0).then_some(blockno);
    }

    None
}

fn bmap_alloc(ip: &mut Inode, bn: u32) -> Option<u32> {
    if bn < NDIRECT as u32 {
        let i = bn as usize;

        if ip.addrs[i] == 0 {
            ip.addrs[i] = balloc()?;
        }

        return Some(ip.addrs[i]);
    }

    let bn = bn - NDIRECT as u32;

    if bn < NINDIRECT as u32 {
        if ip.addrs[NDIRECT] == 0 {
            ip.addrs[NDIRECT] = balloc()?;
        }

        let indirect_block = ip.addrs[NDIRECT];
        let mut buf = [0u8; BSIZE];

        bread(indirect_block, &mut buf);

        let off = bn as usize * core::mem::size_of::<u32>();
        let mut addr = unsafe { core::ptr::read_unaligned(buf.as_ptr().add(off) as *const u32) };

        if addr == 0 {
            addr = balloc()?;
            unsafe {
                core::ptr::write_unaligned(buf.as_mut_ptr().add(off) as *mut u32, addr);
            }
            bwrite(indirect_block, &buf);
        }

        return Some(addr);
    }

    None
}

fn iupdate(ip: &Inode) {
    let dip = Dinode {
        typ: ip.typ,
        major: ip.major,
        minor: ip.minor,
        nlink: ip.nlink,
        size: ip.size,
        addrs: ip.addrs,
    };

    write_dinode(ip.inum, &dip);
}

fn _readi(ip: &mut Inode, off: u32, dst: &mut [u8]) -> usize {
    if off >= ip.size {
        return 0;
    }

    let n = core::cmp::min(dst.len(), (ip.size - off) as usize);
    let mut total = 0;

    while total < n {
        let cur = off as usize + total;
        let bn = cur / BSIZE;
        let boff = cur % BSIZE;
        let m = core::cmp::min(n - total, BSIZE - boff);

        let blockno = bmap_lookup(ip, bn as u32).expect("readi: hole in non-sparse file");
        let mut buf = [0u8; BSIZE];
        bread(blockno, &mut buf);

        dst[total..total + m].copy_from_slice(&buf[boff..boff + m]);

        total += m;
    }

    total
}

fn zero_fill(ip: &mut Inode, from: u32, to: u32) -> bool {
    if from >= to {
        return true;
    }

    let zero = [0u8; BSIZE];
    let mut total = 0;
    let len = (to - from) as usize;

    while total < len {
        let cur = from as usize + total;
        let bn = cur / BSIZE;
        let boff = cur % BSIZE;
        let m = core::cmp::min(len - total, BSIZE - boff);

        let blockno = match bmap_alloc(ip, bn as u32) {
            Some(b) => b,
            None => return false,
        };

        if boff == 0 && m == BSIZE {
            bwrite(blockno, &zero);
        } else {
            let mut buf = [0u8; BSIZE];
            bread(blockno, &mut buf);
            buf[boff..boff + m].fill(0);
            bwrite(blockno, &buf);
        }

        total += m;
    }

    true
}

fn writei(ip: &mut Inode, off: u32, src: &[u8]) -> usize {
    if src.is_empty() {
        return 0;
    }

    let end = match (off as usize).checked_add(src.len()) {
        Some(end) => end,
        None => return 0,
    };

    if end > MAXFILE * BSIZE {
        return 0;
    }

    let mut size_changed = false;
    if off > ip.size {
        if !zero_fill(ip, ip.size, off) {
            return 0;
        }
        ip.size = off;
        size_changed = true;
    }

    let mut total = 0;

    while total < src.len() {
        let cur = off as usize + total;
        let bn = cur / BSIZE;
        let boff = cur % BSIZE;
        let m = core::cmp::min(src.len() - total, BSIZE - boff);

        let blockno = match bmap_alloc(ip, bn as u32) {
            Some(b) => b,
            None => break,
        };

        let mut buf = [0u8; BSIZE];
        bread(blockno, &mut buf);

        buf[boff..boff + m].copy_from_slice(&src[total..total + m]);

        bwrite(blockno, &buf);

        total += m;
    }

    let new_size = off as usize + total;
    if new_size > ip.size as usize {
        ip.size = new_size as u32;
    }

    if total > 0 || size_changed {
        iupdate(ip);
    }

    total
}

const DIRSIZ: usize = 14;

#[repr(C)]
#[derive(Clone, Copy)]
struct Dirent {
    inum: u16,
    name: [u8; DIRSIZ],
}

fn dirent_name(name: &[u8]) -> [u8; DIRSIZ] {
    let mut out = [0u8; DIRSIZ];
    let n = core::cmp::min(name.len(), DIRSIZ);
    out[..n].copy_from_slice(&name[..n]);
    out
}

fn name_eq(dir_name: &[u8; DIRSIZ], name: &[u8]) -> bool {
    let target = dirent_name(name);
    dir_name == &target
}

fn read_dirent(dp: &mut Inode, off: u32) -> Option<Dirent> {
    let mut buf = [0u8; core::mem::size_of::<Dirent>()];
    if _readi(dp, off, &mut buf) != buf.len() {
        return None;
    }

    Some(unsafe { core::ptr::read_unaligned(buf.as_ptr() as *const Dirent) })
}

fn write_dirent(dp: &mut Inode, off: u32, de: &Dirent) -> bool {
    let mut buf = [0u8; core::mem::size_of::<Dirent>()];

    unsafe {
        core::ptr::write_unaligned(buf.as_mut_ptr() as *mut Dirent, *de);
    }

    writei(dp, off, &buf) == buf.len()
}

fn dirlookup(dp: &mut Inode, name: &[u8]) -> Option<u32> {
    if dp.typ != T_DIR {
        return None;
    }

    let mut off = 0;
    let entsize = core::mem::size_of::<Dirent>() as u32;

    while off < dp.size {
        let de = read_dirent(dp, off)?;

        if de.inum != 0 && name_eq(&de.name, name) {
            return Some(de.inum as u32);
        }

        off += entsize;
    }

    None
}

fn dirlink(dp: &mut Inode, name: &[u8], inum: u32) -> bool {
    if dp.typ != T_DIR {
        return false;
    }

    if dirlookup(dp, name).is_some() {
        return false;
    }

    let entsize = core::mem::size_of::<Dirent>() as u32;
    let mut off = 0;

    while off < dp.size {
        let de = match read_dirent(dp, off) {
            Some(de) => de,
            None => return false,
        };

        if de.inum == 0 {
            break;
        }

        off += entsize;
    }

    let de = Dirent {
        inum: inum as u16,
        name: dirent_name(name),
    };

    write_dirent(dp, off, &de)
}

fn ialloc(typ: i16) -> Option<InodeRef> {
    for inum in 1..NINODES as u32 {
        let mut dip = read_dinode(inum);

        if dip.typ == 0 {
            dip = Dinode {
                typ,
                major: 0,
                minor: 0,
                nlink: 0,
                size: 0,
                addrs: [0; NDIRECT + 1],
            };

            write_dinode(inum, &dip);

            let ip = iget(inum)?;
            return Some(ip);
        }
    }

    None
}

fn inc_nlink(ip_ref: InodeRef) {
    let mut ip = ilock(ip_ref);
    ip.nlink = ip.nlink.checked_add(1).expect("inc_nlink: overflow");
    iupdate(&ip);
}

fn link_child(dp_ref: InodeRef, name: &[u8], child_inum: u32) -> bool {
    let mut dp = ilock(dp_ref);
    dirlink(&mut dp, name, child_inum)
}

fn mkdir(parent_ref: InodeRef, name: &[u8]) -> Option<InodeRef> {
    let parent_inum = {
        let dp = ilock(parent_ref);
        dp.inum
    };

    let ip_ref = ialloc(T_DIR)?;
    let inum;
    let ok;

    {
        let mut ip = ilock(ip_ref);
        inum = ip.inum;

        ok = dirlink(&mut ip, b".", inum) && dirlink(&mut ip, b"..", parent_inum);
        if ok {
            ip.nlink = 1; // "."
            iupdate(&ip);
        }
    }

    if !ok {
        iput(ip_ref);
        return None;
    }

    if !link_child(parent_ref, name, inum) {
        iput(ip_ref);
        return None;
    }

    inc_nlink(ip_ref); // parent directory entry
    inc_nlink(parent_ref); // child's ".."

    Some(ip_ref)
}

fn create_file(parent_ref: InodeRef, name: &[u8], data: &[u8]) -> Option<InodeRef> {
    let ip_ref = ialloc(T_FILE)?;
    let inum;
    let ok;

    {
        let mut ip = ilock(ip_ref);
        inum = ip.inum;
        ok = writei(&mut ip, 0, data) == data.len();
    }

    if !ok {
        iput(ip_ref);
        return None;
    }

    if !link_child(parent_ref, name, inum) {
        iput(ip_ref);
        return None;
    }

    inc_nlink(ip_ref);

    Some(ip_ref)
}

fn write_superblock() {
    let mut buf = [0u8; BSIZE];

    unsafe {
        core::ptr::write_unaligned(buf.as_mut_ptr() as *mut SuperBlock, SB);
    }

    bwrite(1, &buf);
}

fn init_root() -> Option<InodeRef> {
    let dip = Dinode {
        typ: T_DIR,
        major: 0,
        minor: 0,
        nlink: 0,
        size: 0,
        addrs: [0; NDIRECT + 1],
    };

    write_dinode(ROOTINO, &dip);

    iget(ROOTINO)
}

fn populate_root() {
    write_superblock();

    let root = init_root().expect("init root");

    {
        let mut root_dp = ilock(root);

        if dirlink(&mut root_dp, b".", ROOTINO) {
            root_dp.nlink += 1;
        }
        if dirlink(&mut root_dp, b"..", ROOTINO) {
            root_dp.nlink += 1;
        }
        iupdate(&root_dp);
    }

    let bin = mkdir(root, b"bin").expect("mkdir /bin");
    let sh = create_file(
        bin,
        b"sh",
        include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/sh"),
    )
    .expect("create /bin/sh");
    iput(sh);

    let cat = create_file(
        bin,
        b"cat",
        include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/cat"),
    )
    .expect("create /bin/cat");
    iput(cat);

    let ls = create_file(
        bin,
        b"ls",
        include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/ls"),
    )
    .expect("create /bin/ls");
    iput(ls);

    let alloc_test = create_file(
        bin,
        b"alloc_test",
        include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/alloc_test"),
    )
    .expect("create /bin/alloc_test");
    iput(alloc_test);

    let readme = create_file(root, b"README.md", include_str!("../README.md").as_bytes())
        .expect("create README.md");
    iput(readme);

    iput(bin);
    iput(root);
}

fn skipelem(mut path: &[u8]) -> Option<(&[u8], &[u8])> {
    while matches!(path.first(), Some(b'/')) {
        path = &path[1..];
    }

    if path.is_empty() {
        return None;
    }

    match path.iter().position(|&b| b == b'/') {
        Some(i) => Some((&path[..i], &path[i..])),
        None => Some((path, &[])),
    }
}

pub fn namei(cwd: InodeRef, path: &[u8]) -> Option<InodeRef> {
    if path.is_empty() {
        return None;
    }

    let mut ip_ref = if path[0] == b'/' {
        iget(ROOTINO)?
    } else {
        idup(cwd)
    };

    let mut rest = path;
    while let Some((name, next)) = skipelem(rest) {
        let inum = {
            let mut ip = ilock(ip_ref);
            if ip.typ != T_DIR {
                None
            } else {
                dirlookup(&mut ip, name)
            }
        };

        let inum = match inum {
            Some(inum) => inum,
            None => {
                iput(ip_ref);
                return None;
            }
        };

        let next_ref = match iget(inum) {
            Some(next_ref) => next_ref,
            None => {
                iput(ip_ref);
                return None;
            }
        };

        iput(ip_ref);
        ip_ref = next_ref;
        rest = next;
    }

    Some(ip_ref)
}

pub const fn root_placeholder() -> InodeRef {
    &ICACHE[0]
}

pub fn root() -> InodeRef {
    iget(ROOTINO).expect("root")
}

pub fn init() {
    unsafe {
        core::ptr::write_bytes(core::ptr::addr_of_mut!(DISK) as *mut u8, 0, BSIZE * FSSIZE);
    }

    ICACHE_LOCK.acquire();
    for slot in ICACHE.iter() {
        let mut ip = slot.lock();
        *ip = Inode::empty();
    }
    ICACHE_LOCK.release();

    populate_root();
}

pub fn inode_type(ip_ref: InodeRef) -> InodeType {
    let ip = ilock(ip_ref);
    match ip.typ {
        T_FILE => InodeType::File,
        T_DIR => InodeType::Dir,
        T_DEVICE => InodeType::Device {
            major: ip.major as u16,
        },
        _ => InodeType::File,
    }
}

pub fn stati(ip_ref: InodeRef) -> Stat {
    let ip = ilock(ip_ref);
    Stat {
        typ: ip.typ,
        ino: ip.inum,
        nlink: ip.nlink,
        size: ip.size,
    }
}

pub fn readi(ip_ref: InodeRef, off: usize, dst: &mut [u8]) -> isize {
    let Ok(off) = u32::try_from(off) else {
        return -1;
    };

    let mut ip = ilock(ip_ref);
    _readi(&mut ip, off, dst) as isize
}

pub fn selftest() {
    test_bread_bwrite();
    test_dinode_rw();
    test_balloc_bfree();
    test_inode_cache_refcnt();
    test_readi_writei();
    test_directory_layer();
    test_ialloc();
    test_populate_root();
}

fn test_bread_bwrite() {
    let blockno = 10;
    let mut w = [0u8; BSIZE];
    let mut r = [0u8; BSIZE];

    w[0] = 0x12;
    w[1] = 0x34;
    w[BSIZE - 1] = 0xab;

    bwrite(blockno, &w);
    bread(blockno, &mut r);

    assert!(r[0] == 0x12);
    assert!(r[1] == 0x34);
    assert!(r[BSIZE - 1] == 0xab);
}

fn test_dinode_rw() {
    let a = Dinode {
        typ: 1,
        major: 0,
        minor: 0,
        nlink: 1,
        size: 123,
        addrs: [0; NDIRECT + 1],
    };

    let b = Dinode {
        typ: 2,
        major: 0,
        minor: 0,
        nlink: 1,
        size: 456,
        addrs: [0; NDIRECT + 1],
    };

    write_dinode(ROOTINO, &a);
    write_dinode(ROOTINO + 1, &b);

    let got_a = read_dinode(ROOTINO);
    let got_b = read_dinode(ROOTINO + 1);

    assert!(got_a.typ == 1);
    assert!(got_a.size == 123);
    assert!(got_b.typ == 2);
    assert!(got_b.size == 456);
}

fn test_balloc_bfree() {
    let first = balloc().expect("balloc first");
    let second = balloc().expect("balloc second");

    assert!(first as usize == DATASTART);
    assert!(second as usize == DATASTART + 1);

    let mut pattern = [0xffu8; BSIZE];
    bwrite(first, &pattern);

    bfree(first);

    let reused = balloc().expect("balloc reused");
    assert!(reused == first);

    bread(reused, &mut pattern);
    for byte in pattern {
        assert!(byte == 0);
    }
}

fn test_inode_cache_refcnt() {
    let inum = ROOTINO + 2;
    let dip = Dinode {
        typ: 2,
        major: 0,
        minor: 0,
        nlink: 1,
        size: 777,
        addrs: [0; NDIRECT + 1],
    };

    write_dinode(inum, &dip);

    let ip1 = iget(inum).expect("iget first");
    let ip2 = iget(inum).expect("iget second");

    assert!(core::ptr::eq(ip1, ip2));

    {
        let ip = ilock(ip1);
        assert!(ip.valid);
        assert!(ip.typ == 2);
        assert!(ip.size == 777);
        assert!(ip.refcnt == 2);
    }

    let ip3 = idup(ip1);
    assert!(core::ptr::eq(ip1, ip3));

    {
        let ip = ip1.lock();
        assert!(ip.refcnt == 3);
    }

    iput(ip1);
    iput(ip2);
    iput(ip3);

    {
        let ip = ip1.lock();
        assert!(ip.refcnt == 0);
        assert!(ip.valid);
        assert!(ip.inum == inum);
    }
}

fn test_readi_writei() {
    let inum = ROOTINO + 3;
    let dip = Dinode {
        typ: 2,
        major: 0,
        minor: 0,
        nlink: 1,
        size: 0,
        addrs: [0; NDIRECT + 1],
    };

    write_dinode(inum, &dip);

    let ip = iget(inum).expect("iget readi/writei");

    {
        let mut ip = ilock(ip);

        let mut small = [0u8; 8];
        assert!(_readi(&mut ip, 0, &mut small) == 0);

        assert!(writei(&mut ip, 0, b"hello") == 5);
        assert!(ip.size == 5);

        assert!(_readi(&mut ip, 0, &mut small) == 5);
        assert!(&small[..5] == b"hello");

        let mut tail = [0u8; 4];
        assert!(_readi(&mut ip, 1, &mut tail) == 4);
        assert!(&tail == b"ello");

        assert!(writei(&mut ip, 1, b"AB") == 2);
        assert!(ip.size == 5);

        assert!(_readi(&mut ip, 0, &mut small) == 5);
        assert!(&small[..5] == b"hABlo");

        let cross = *b"0123456789abcdef0123456789abcdef";
        let off = (BSIZE - 8) as u32;
        assert!(writei(&mut ip, off, &cross) == cross.len());

        let mut got = [0u8; 32];
        assert!(_readi(&mut ip, off, &mut got) == got.len());
        assert!(got == cross);

        let mut gap = [0xffu8; 16];
        assert!(_readi(&mut ip, 5, &mut gap) == gap.len());
        assert!(gap == [0u8; 16]);

        let mut before_cross = [0xffu8; 16];
        assert!(_readi(&mut ip, (BSIZE - 16) as u32, &mut before_cross) == before_cross.len());
        assert!(before_cross[..8] == [0u8; 8]);
        assert!(before_cross[8..] == cross[..8]);
    }

    iput(ip);

    let ip = iget(inum).expect("iget readback");
    {
        let mut ip = ilock(ip);
        let mut got = [0u8; 5];

        assert!(ip.size as usize == BSIZE - 8 + 32);
        assert!(_readi(&mut ip, 0, &mut got) == got.len());
        assert!(got == *b"hABlo");
    }
    iput(ip);
}

fn test_directory_layer() {
    let dir_inum = ROOTINO + 4;
    let child_inum = ROOTINO + 5;
    let dip = Dinode {
        typ: T_DIR,
        major: 0,
        minor: 0,
        nlink: 1,
        size: 0,
        addrs: [0; NDIRECT + 1],
    };

    write_dinode(dir_inum, &dip);

    let dp = iget(dir_inum).expect("iget dir");
    {
        let mut dp = ilock(dp);

        assert!(dirlookup(&mut dp, b"foo").is_none());
        assert!(dirlink(&mut dp, b"foo", child_inum));
        assert!(dirlookup(&mut dp, b"foo") == Some(child_inum));
        assert!(!dirlink(&mut dp, b"foo", child_inum + 1));

        assert!(dirlink(&mut dp, b"bar", child_inum + 1));
        assert!(dirlookup(&mut dp, b"bar") == Some(child_inum + 1));
    }
    iput(dp);

    let dp = iget(dir_inum).expect("iget dir readback");
    {
        let mut dp = ilock(dp);

        assert!(dp.typ == T_DIR);
        assert!(dirlookup(&mut dp, b"foo") == Some(child_inum));
        assert!(dirlookup(&mut dp, b"bar") == Some(child_inum + 1));
        assert!(dirlookup(&mut dp, b"missing").is_none());
    }
    iput(dp);
}

fn test_ialloc() {
    let ip = ialloc(T_FILE).expect("ialloc file");
    let inum;

    {
        let ip = ilock(ip);
        inum = ip.inum;

        assert!(inum > ROOTINO);
        assert!(ip.typ == T_FILE);
        assert!(ip.major == 0);
        assert!(ip.minor == 0);
        assert!(ip.nlink == 0);
        assert!(ip.size == 0);
        for addr in ip.addrs {
            assert!(addr == 0);
        }
    }

    let dip = read_dinode(inum);
    assert!(dip.typ == T_FILE);
    assert!(dip.nlink == 0);
    assert!(dip.size == 0);

    iput(ip);

    let next = ialloc(T_DIR).expect("ialloc dir");
    {
        let ip = ilock(next);
        assert!(ip.inum != inum);
        assert!(ip.typ == T_DIR);
    }
    iput(next);
}

fn test_populate_root() {
    populate_root();

    let root = iget(ROOTINO).expect("iget root");
    {
        let mut dp = ilock(root);

        assert!(dp.typ == T_DIR);
        assert!(dp.nlink == 3);
        assert!(dirlookup(&mut dp, b".") == Some(ROOTINO));
        assert!(dirlookup(&mut dp, b"..") == Some(ROOTINO));
    }

    let root_by_name = namei(root, b"/").expect("namei /");
    {
        let ip = ilock(root_by_name);
        assert!(ip.inum == ROOTINO);
        assert!(ip.typ == T_DIR);
    }
    iput(root_by_name);

    let bin = namei(root, b"/bin").expect("namei /bin");
    {
        let ip = ilock(bin);
        assert!(ip.typ == T_DIR);
        assert!(ip.nlink == 2);
    }

    let bin_relative = namei(root, b"bin").expect("namei relative bin");
    assert!(core::ptr::eq(bin, bin_relative));
    iput(bin_relative);

    let parent = namei(bin, b"..").expect("namei bin/..");
    {
        let ip = ilock(parent);
        assert!(ip.inum == ROOTINO);
    }
    iput(parent);
    iput(bin);

    let readme = namei(root, b"/README.md").expect("namei /README.md");
    {
        let mut ip = ilock(readme);
        let expected = include_str!("../README.md").as_bytes();
        let mut buf = [0u8; 16];
        let n = core::cmp::min(buf.len(), expected.len());

        assert!(ip.typ == T_FILE);
        assert!(ip.nlink == 1);
        assert!(_readi(&mut ip, 0, &mut buf[..n]) == n);
        assert!(&buf[..n] == &expected[..n]);
    }
    iput(readme);

    assert!(namei(root, b"/missing").is_none());
    assert!(namei(root, b"").is_none());

    iput(root);
}

pub enum InodeType {
    File,
    Dir,
    Device { major: u16 },
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Stat {
    pub typ: i16,
    pub ino: u32,
    pub nlink: i16,
    pub size: u32,
}
