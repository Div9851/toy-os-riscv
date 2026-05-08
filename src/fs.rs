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

static READ_LINE: Inode = Inode {
    kind: InodeKind::File {
        data: include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/read_line"),
    },
};

static READ_FILE: Inode = Inode {
    kind: InodeKind::File {
        data: include_bytes!("../user/target/riscv64gc-unknown-none-elf/release/read_file"),
    },
};

static BIN_ENTRIES: [DirEnt; 2] = [
    DirEnt {
        name: b"read_line",
        inode: &READ_LINE,
    },
    DirEnt {
        name: b"read_file",
        inode: &READ_FILE,
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

static ROOT_ENTRIES: [DirEnt; 2] = [
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

pub fn namei(path: &[u8]) -> Option<InodeRef> {
    if path.is_empty() || path[0] != b'/' {
        return None;
    }

    let mut cur = &ROOT;
    let rest = &path[1..];

    if rest.is_empty() {
        return Some(&ROOT);
    }

    if rest.ends_with(b"/") {
        return None;
    }

    for component in rest.split(|b| *b == b'/') {
        if component.is_empty() {
            return None;
        }

        cur = dir_lookup(cur, component)?;
    }

    Some(cur)
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
