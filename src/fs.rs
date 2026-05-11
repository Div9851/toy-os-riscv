pub struct Inode {
    kind: InodeKind,
}

impl Inode {
    pub fn inode_type(&self) -> InodeType {
        match &self.kind {
            InodeKind::File { .. } => InodeType::File,
            InodeKind::Dir { .. } => InodeType::Dir,
            InodeKind::Device { major } => InodeType::Device { major: *major },
        }
    }
}

pub type InodeRef = &'static Inode;

enum InodeKind {
    File { data: &'static [u8] },
    Dir { entries: &'static [DirEnt] },
    Device { major: u16 },
}

pub enum InodeType {
    File,
    Dir,
    Device { major: u16 },
}

struct DirEnt {
    name: &'static [u8],
    inode: InodeRef,
}

static CAT: Inode = Inode {
    kind: InodeKind::File {
        data: include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/cat"),
    },
};

static ALLOC_TEST: Inode = Inode {
    kind: InodeKind::File {
        data: include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/alloc_test"),
    },
};

static SH: Inode = Inode {
    kind: InodeKind::File {
        data: include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/sh"),
    },
};

static BIN_ENTRIES: [DirEnt; 5] = [
    DirEnt {
        name: b".",
        inode: &BIN,
    },
    DirEnt {
        name: b"..",
        inode: &ROOT,
    },
    DirEnt {
        name: b"alloc_test",
        inode: &ALLOC_TEST,
    },
    DirEnt {
        name: b"cat",
        inode: &CAT,
    },
    DirEnt {
        name: b"sh",
        inode: &SH,
    },
];

static BIN: Inode = Inode {
    kind: InodeKind::Dir {
        entries: &BIN_ENTRIES,
    },
};

static README: Inode = Inode {
    kind: InodeKind::File {
        data: include_str!("../README.md").as_bytes(),
    },
};

static ROOT_ENTRIES: [DirEnt; 4] = [
    DirEnt {
        name: b".",
        inode: &ROOT,
    },
    DirEnt {
        name: b"..",
        inode: &ROOT,
    },
    DirEnt {
        name: b"bin",
        inode: &BIN,
    },
    DirEnt {
        name: b"README.md",
        inode: &README,
    },
];

static ROOT: Inode = Inode {
    kind: InodeKind::Dir {
        entries: &ROOT_ENTRIES,
    },
};

pub const fn root() -> InodeRef {
    &ROOT
}

pub fn namei_at(cwd: InodeRef, path: &[u8]) -> Option<InodeRef> {
    if path.is_empty() {
        return None;
    }

    let mut cur = if path[0] == b'/' { root() } else { cwd };

    let mut rest = path;

    while let Some((name, next)) = next_component(rest) {
        cur = dir_lookup(cur, name)?;
        rest = next;
    }

    Some(cur)
}

fn next_component(mut path: &[u8]) -> Option<(&[u8], &[u8])> {
    while matches!(path.first(), Some(b'/')) {
        path = &path[1..];
    }
    if path.is_empty() {
        return None;
    }

    let slash = path.iter().position(|&b| b == b'/');

    match slash {
        Some(i) => Some((&path[..i], &path[i..])),
        None => Some((path, &[])),
    }
}

fn dir_lookup(inode: InodeRef, name: &[u8]) -> Option<InodeRef> {
    match &inode.kind {
        InodeKind::Dir { entries } => {
            for entry in *entries {
                if entry.name == name {
                    return Some(entry.inode);
                }
            }
            None
        }
        _ => None,
    }
}

pub fn readi(inode: InodeRef, off: usize, dst: &mut [u8]) -> isize {
    match &inode.kind {
        InodeKind::File { data } => {
            if off >= data.len() {
                return 0;
            }

            let n = core::cmp::min(data.len() - off, dst.len());
            dst[..n].copy_from_slice(&data[off..off + n]);
            n as isize
        }
        _ => -1,
    }
}
