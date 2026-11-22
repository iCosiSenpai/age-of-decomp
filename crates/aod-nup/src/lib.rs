//! Read-only reader for NUP containers: unencrypted Nintendo Switch / Switch 2 dumps (format spec:
//! <https://github.com/a55uka/nup/blob/main/SPEC.md>, v1.0-draft). Every offset is bounds-checked, the TOC hash is
//! verified before anything in it is trusted, and file names are labels, never paths.
//!
//! Also holds the Age of Decomp game-profile check ([`profile`]): the exact build the engine supports.

pub mod profile;

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const SUPERBLOCK_SIZE: usize = 0x80;
const PACKAGE_SIZE: usize = 0x40;
const DIR_SIZE: usize = 0x18;
const FILE_SIZE: usize = 0x50;
const TOC_HEADER: usize = 0x30;
/// TOCs larger than this are refused (the HWAoI TOC is 389 KiB for 4228 files).
const MAX_TOC: u64 = 256 << 20;
/// A dump holds the base game, its update and DLC: a few packages. Their metadata all stays in memory.
const MAX_PACKAGES: usize = 256;
const MAX_META_TOTAL: u64 = 64 << 20;
const MAX_READ_ALL: u64 = 512 << 20;
const MAX_READ_RESERVE: u64 = 64 << 20;
const NONE: u32 = u32::MAX;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("not a NUP package (wrong header)")]
    NotNup,
    #[error("this NUP uses a format version or features this reader does not know")]
    Unsupported,
    #[error("the NUP is damaged or incomplete: {0}")]
    Corrupt(String),
    #[error("zstd: {0}")]
    Zstd(String),
}

pub type Result<T> = std::result::Result<T, Error>;

fn corrupt<T>(what: impl Into<String>) -> Result<T> {
    Err(Error::Corrupt(what.into()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Base,
    Update,
    Addon,
    AddonUpdate,
    System,
}

impl Kind {
    fn from_u8(v: u8) -> Result<Kind> {
        Ok(match v {
            0 => Kind::Base,
            1 => Kind::Update,
            2 => Kind::Addon,
            3 => Kind::AddonUpdate,
            4 => Kind::System,
            _ => return Err(Error::Unsupported),
        })
    }
    pub fn is_program(self) -> bool {
        matches!(self, Kind::Base | Kind::Update)
    }
}

/// Package metadata from the optional TLV block.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Meta {
    pub name: Option<String>,
    pub publisher: Option<String>,
    pub display_version: Option<String>,
    pub source_dump: Option<String>,
    /// NSO ModuleId of `exefs/main`, hex, when the producer stored it.
    pub build_id: Option<String>,
    #[serde(skip)]
    pub icon: Option<Vec<u8>>,
    #[serde(skip)]
    pub npdm: Option<Vec<u8>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Package {
    pub index: usize,
    pub title_id: u64,
    pub version: u32,
    pub kind: Kind,
    pub standalone_update: bool,
    #[serde(skip)]
    pub root_dir: u32,
    #[serde(skip)]
    pub exefs_dir: Option<u32>,
    #[serde(skip)]
    pub romfs_dir: Option<u32>,
    pub meta: Meta,
}

/// A file node of the TOC.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub extent_offset: u64,
    pub extent_size: u64,
    pub sha256: [u8; 32],
    pub codec: u8,
}

#[derive(Clone, Debug)]
pub enum Node {
    Dir(u32),
    File(u32),
}

/// What the engine records to recognise the same NUP later (upstream `identity_of`).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Identity {
    pub header: String,
    pub toc: String,
    pub size: u64,
}

pub struct Nup {
    path: PathBuf,
    file: Mutex<File>,
    file_size: u64,
    header_sha256: [u8; 32],
    toc_sha256: [u8; 32],
    toc: Vec<u8>,
    dir_count: u64,
    file_count: u64,
    dirs_at: usize,
    files_at: usize,
    children_at: usize,
    pool: (usize, usize),
    packages: Vec<Package>,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

impl Nup {
    /// Opens and validates a NUP: superblock rules, file size, TOC bounds and SHA-256, every package record.
    pub fn open(path: impl AsRef<Path>) -> Result<Nup> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let file_size = file.metadata()?.len();
        if file_size < SUPERBLOCK_SIZE as u64 {
            return Err(Error::NotNup);
        }
        let mut sb = [0u8; SUPERBLOCK_SIZE];
        file.read_exact(&mut sb)?;
        if &sb[0..3] != b"NUP" {
            return Err(Error::NotNup);
        }
        if sb[3] != 1 {
            return Err(Error::Unsupported);
        }
        let header_size = u16_at(&sb, 4) as u64;
        let toc_offset = u64_at(&sb, 0x08);
        let toc_size = u64_at(&sb, 0x10);
        let blob_offset = u64_at(&sb, 0x38);
        let declared_size = u64_at(&sb, 0x40);
        let package_count = u32_at(&sb, 0x48) as usize;
        let flags = u32_at(&sb, 0x4C);
        if header_size < SUPERBLOCK_SIZE as u64 || toc_offset != header_size {
            return corrupt("bad superblock");
        }
        if flags != 0 {
            return Err(Error::Unsupported);
        }
        if declared_size != file_size {
            return corrupt(format!("size mismatch: header says {declared_size} bytes, file has {file_size}"));
        }
        if toc_size > MAX_TOC || !blob_offset.is_multiple_of(16) || blob_offset < toc_offset + toc_size || blob_offset > file_size {
            return corrupt("bad TOC or blob bounds");
        }
        if package_count == 0 {
            return corrupt("no packages");
        }
        if package_count > MAX_PACKAGES {
            return corrupt("too many packages");
        }
        let header_sha256: [u8; 32] = Sha256::digest(sb).into();
        file.seek(SeekFrom::Start(toc_offset))?;
        let mut toc = vec![0u8; toc_size as usize];
        file.read_exact(&mut toc)?;
        let toc_sha256: [u8; 32] = Sha256::digest(&toc).into();
        if toc_sha256[..] != sb[0x18..0x38] {
            return corrupt("TOC SHA-256 mismatch");
        }
        if toc.len() < TOC_HEADER || &toc[0..4] != b"TOC\x01" {
            return corrupt("bad TOC header");
        }
        if u32_at(&toc, 4) as usize != package_count {
            return corrupt("package count mismatch");
        }
        let dir_count = u64_at(&toc, 0x08);
        let file_count = u64_at(&toc, 0x10);
        let pool_off = u64_at(&toc, 0x18);
        let pool_size = u64_at(&toc, 0x20);
        let dirs_at = TOC_HEADER + package_count * PACKAGE_SIZE;
        let files_at = (dirs_at as u64).checked_add(dir_count.saturating_mul(DIR_SIZE as u64));
        let children_at = files_at.and_then(|f| f.checked_add(file_count.checked_mul(FILE_SIZE as u64)?));
        let (Some(files_at), Some(children_at)) = (files_at, children_at) else {
            return corrupt("TOC counts overflow");
        };
        if children_at > toc_size || pool_off.checked_add(pool_size).is_none_or(|end| end > toc_size) {
            return corrupt("TOC tables out of bounds");
        }
        let mut nup = Nup {
            path,
            file: Mutex::new(file),
            file_size,
            header_sha256,
            toc_sha256,
            toc,
            dir_count,
            file_count,
            dirs_at,
            files_at: files_at as usize,
            children_at: children_at as usize,
            pool: (pool_off as usize, pool_size as usize),
            packages: Vec::new(),
        };
        let meta_total: u64 = (0..package_count).map(|i| u32_at(&nup.toc, TOC_HEADER + i * PACKAGE_SIZE + 0x28) as u64).sum();
        if meta_total > MAX_META_TOTAL {
            return corrupt("package metadata too large");
        }
        for index in 0..package_count {
            let pkg = nup.read_package(index)?;
            nup.packages.push(pkg);
        }
        Ok(nup)
    }

    fn read_package(&self, index: usize) -> Result<Package> {
        let o = TOC_HEADER + index * PACKAGE_SIZE;
        let t = &self.toc;
        let dir = |v: u32| -> Result<Option<u32>> {
            if v == NONE {
                Ok(None)
            } else if (v as u64) < self.dir_count {
                Ok(Some(v))
            } else {
                corrupt("package refers to a folder that does not exist")
            }
        };
        let root_dir = dir(u32_at(t, o + 0x08))?.ok_or_else(|| Error::Corrupt("package without root".into()))?;
        let pkg_flags = t[o + 0x19];
        if pkg_flags & !1 != 0 {
            return Err(Error::Unsupported);
        }
        let meta_offset = u64_at(t, o + 0x20);
        let meta_size = u32_at(t, o + 0x28) as u64;
        let meta = if meta_offset != 0 && meta_size != 0 {
            if meta_offset.checked_add(meta_size).is_none_or(|e| e > self.file_size) || meta_size > (16 << 20) {
                return corrupt("meta block out of bounds");
            }
            let mut raw = vec![0u8; meta_size as usize];
            let mut f = self.file.lock().unwrap();
            f.seek(SeekFrom::Start(meta_offset))?;
            f.read_exact(&mut raw)?;
            parse_meta(&raw)
        } else {
            Meta::default()
        };
        Ok(Package {
            index,
            title_id: u64_at(t, o),
            root_dir,
            exefs_dir: dir(u32_at(t, o + 0x0C))?,
            romfs_dir: dir(u32_at(t, o + 0x10))?,
            version: u32_at(t, o + 0x14),
            kind: Kind::from_u8(t[o + 0x18])?,
            standalone_update: pkg_flags & 1 != 0,
            meta,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn file_size(&self) -> u64 {
        self.file_size
    }
    pub fn packages(&self) -> &[Package] {
        &self.packages
    }
    pub fn identity(&self) -> Identity {
        Identity { header: hex::encode(self.header_sha256), toc: hex::encode(self.toc_sha256), size: self.file_size }
    }

    /// The program package that runs: the update with the highest version, else the base.
    pub fn newest_program(&self) -> Option<&Package> {
        self.packages.iter().filter(|p| p.kind.is_program()).max_by_key(|p| (p.kind == Kind::Update, p.version))
    }

    fn name(&self, off: u32, len: u32) -> Result<String> {
        let (pool_off, pool_size) = self.pool;
        let (off, len) = (off as usize, len as usize);
        if off.checked_add(len).is_none_or(|e| e > pool_size) {
            return corrupt("a name is out of bounds");
        }
        Ok(String::from_utf8_lossy(&self.toc[pool_off + off..pool_off + off + len]).into_owned())
    }

    pub fn dir_name(&self, dir: u32) -> Result<String> {
        if dir as u64 >= self.dir_count {
            return corrupt("folder index out of bounds");
        }
        let o = self.dirs_at + dir as usize * DIR_SIZE;
        self.name(u32_at(&self.toc, o), u32_at(&self.toc, o + 4))
    }

    pub fn entry(&self, file: u32) -> Result<Entry> {
        if file as u64 >= self.file_count {
            return corrupt("file index out of bounds");
        }
        let o = self.files_at + file as usize * FILE_SIZE;
        let t = &self.toc;
        let e = Entry {
            name: self.name(u32_at(t, o), u32_at(t, o + 4))?,
            size: u64_at(t, o + 0x08),
            extent_offset: u64_at(t, o + 0x10),
            extent_size: u64_at(t, o + 0x18),
            sha256: t[o + 0x20..o + 0x40].try_into().unwrap(),
            codec: t[o + 0x40],
        };
        if e.codec > 1 {
            return Err(Error::Unsupported);
        }
        if e.extent_size != 0 && e.extent_offset.checked_add(e.extent_size).is_none_or(|end| end > self.file_size) {
            return corrupt(format!("{} points outside the file", e.name));
        }
        if e.codec == 0 && e.extent_size != e.size {
            return corrupt(format!("{}: raw size mismatch", e.name));
        }
        Ok(e)
    }

    /// The children of a folder, in TOC order (bytewise name order).
    pub fn children(&self, dir: u32) -> Result<Vec<Node>> {
        if dir as u64 >= self.dir_count {
            return corrupt("folder index out of bounds");
        }
        let o = self.dirs_at + dir as usize * DIR_SIZE;
        let first = u32_at(&self.toc, o + 8) as usize;
        let count = u32_at(&self.toc, o + 12) as usize;
        let end = self.children_at as u64 + (first as u64 + count as u64) * 4;
        // The children array ends where the name pool starts when the pool follows it (where the format asks
        // producers to put it), else at the end of the table of contents: a listing never reads names as node ids.
        let limit = if self.pool.0 >= self.children_at { self.pool.0 } else { self.toc.len() };
        if end > limit as u64 {
            return corrupt("folder listing out of bounds");
        }
        (0..count)
            .map(|k| {
                let at = self.children_at + (first + k) * 4;
                let id = u32_at(&self.toc, at) as u64;
                if id < self.dir_count {
                    Ok(Node::Dir(id as u32))
                } else if id - self.dir_count < self.file_count {
                    Ok(Node::File((id - self.dir_count) as u32))
                } else {
                    corrupt("folder refers to a missing node")
                }
            })
            .collect()
    }

    /// A file directly inside `dir` by exact name.
    pub fn lookup(&self, dir: u32, name: &str) -> Result<Option<Entry>> {
        for node in self.children(dir)? {
            if let Node::File(f) = node {
                let e = self.entry(f)?;
                if e.name == name {
                    return Ok(Some(e));
                }
            }
        }
        Ok(None)
    }

    /// Every file under `dir` with its path relative to `dir` ("a/b/c"), depth-first, folders before deeper levels.
    pub fn walk(&self, dir: u32) -> Result<Vec<(String, Entry)>> {
        if dir as u64 >= self.dir_count {
            return corrupt("folder refers to a missing node");
        }
        let mut visited = vec![0u8; (self.dir_count as usize).div_ceil(8)];
        visited[dir as usize / 8] |= 1 << (dir % 8);
        let mut out = Vec::new();
        let mut stack = vec![(dir, String::new(), 0usize)];
        while let Some((d, prefix, depth)) = stack.pop() {
            if depth > 64 {
                return corrupt("folder tree too deep");
            }
            for node in self.children(d)? {
                match node {
                    Node::Dir(c) => {
                        let byte = &mut visited[c as usize / 8];
                        let bit = 1 << (c % 8);
                        if *byte & bit != 0 {
                            return corrupt("folder listed twice");
                        }
                        *byte |= bit;
                        let name = self.dir_name(c)?;
                        stack.push((c, format!("{prefix}{name}/"), depth + 1));
                    }
                    Node::File(f) => {
                        if out.len() as u64 >= self.file_count {
                            return corrupt("too many files listed");
                        }
                        let e = self.entry(f)?;
                        out.push((format!("{prefix}{}", e.name), e));
                    }
                }
            }
        }
        Ok(out)
    }

    /// The first `n` bytes of a file (decompressing only what is needed).
    pub fn read_head(&self, e: &Entry, n: usize) -> Result<Vec<u8>> {
        let n = n.min(e.size as usize);
        if n == 0 {
            return Ok(Vec::new());
        }
        let mut f = self.file.lock().unwrap();
        f.seek(SeekFrom::Start(e.extent_offset))?;
        let mut out = vec![0u8; n];
        if e.codec == 0 {
            f.read_exact(&mut out)?;
        } else {
            let limited = (&mut *f).take(e.extent_size);
            let mut dec = ruzstd::decoding::StreamingDecoder::new(limited).map_err(|x| Error::Zstd(x.to_string()))?;
            dec.read_exact(&mut out)?;
        }
        Ok(out)
    }

    /// A whole file, decompressed and checked against its SHA-256.
    pub fn read_all(&self, e: &Entry) -> Result<Vec<u8>> {
        if e.size > MAX_READ_ALL {
            return corrupt(format!("{} is too large to read whole", e.name));
        }
        let mut out = Vec::with_capacity(e.size.min(MAX_READ_RESERVE) as usize);
        if e.size != 0 || e.extent_size != 0 {
            let mut f = self.file.lock().unwrap();
            f.seek(SeekFrom::Start(e.extent_offset))?;
            let limited = (&mut *f).take(e.extent_size);
            if e.codec == 0 {
                let mut r = limited.take(e.size + 1);
                r.read_to_end(&mut out)?;
            } else {
                let dec =
                    ruzstd::decoding::StreamingDecoder::new(limited).map_err(|x| Error::Zstd(x.to_string()))?;
                dec.take(e.size + 1).read_to_end(&mut out)?;
            }
        }
        if out.len() as u64 != e.size {
            return corrupt(format!("{}: size mismatch after decompression", e.name));
        }
        let got: [u8; 32] = Sha256::digest(&out).into();
        if got != e.sha256 {
            return corrupt(format!("{}: SHA-256 mismatch", e.name));
        }
        Ok(out)
    }

    /// The NSO ModuleId (build ID) of `exefs/main`: bytes 0x40..0x60 of its header, trailing zeros trimmed to 20 bytes
    /// when they are zero (GNU build IDs are 20 bytes). Falls back to the meta TLV.
    pub fn build_id(&self, pkg: &Package) -> Result<Option<String>> {
        if let Some(exefs) = pkg.exefs_dir
            && let Some(main) = self.lookup(exefs, "main")?
        {
            let head = self.read_head(&main, 0x60)?;
            if head.len() == 0x60 && (&head[0..4] == b"NSO0" || &head[0..4] == b"NRO0") {
                let id = &head[0x40..0x60];
                let used = if id[20..].iter().all(|b| *b == 0) { &id[..20] } else { id };
                return Ok(Some(hex::encode(used)));
            }
        }
        Ok(pkg.meta.build_id.clone())
    }

    /// Folders, files and uncompressed bytes under a package's RomFS.
    pub fn romfs_stats(&self, pkg: &Package) -> Result<(usize, u64)> {
        match pkg.romfs_dir {
            None => Ok((0, 0)),
            Some(d) => {
                let files = self.walk(d)?;
                Ok((files.len(), files.iter().map(|(_, e)| e.size).sum()))
            }
        }
    }
}

fn parse_meta(raw: &[u8]) -> Meta {
    let mut m = Meta::default();
    let mut pos = 0usize;
    while pos + 8 <= raw.len() {
        let kind = u16_at(raw, pos);
        let size = u32_at(raw, pos + 4) as usize;
        let Some(end) = (pos + 8).checked_add(size).filter(|e| *e <= raw.len()) else { break };
        let payload = &raw[pos + 8..end];
        let text = || Some(String::from_utf8_lossy(payload).into_owned());
        match kind {
            0x0001 => m.npdm = Some(payload.to_vec()),
            0x0003 => m.icon = Some(payload.to_vec()),
            0x0004 => m.display_version = text(),
            0x0005 => m.name = text(),
            0x0006 => m.publisher = text(),
            0x0007 if payload.len() == 32 => m.build_id = Some(hex::encode(payload)),
            0x0008 => m.source_dump = text(),
            _ => {}
        }
        pos = end;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal NUP built by hand per SPEC.md: one base package, root/exefs folders, one raw `main` NSO header.
    fn synthetic() -> Vec<u8> {
        let mut main = vec![0u8; 0x60];
        main[..4].copy_from_slice(b"NSO0");
        for (i, b) in main[0x40..0x54].iter_mut().enumerate() {
            *b = i as u8 + 1;
        }
        let names = b"exefsmain";
        let (pkgs, dirs, files) = (1usize, 2usize, 1usize);
        let children_at = 0x30 + pkgs * 0x40 + dirs * 0x18 + files * 0x50;
        let pool_at = children_at + 2 * 4;
        let toc_len = pool_at + names.len();
        let mut toc = vec![0u8; toc_len];
        toc[0..4].copy_from_slice(b"TOC\x01");
        toc[4..8].copy_from_slice(&1u32.to_le_bytes());
        toc[8..16].copy_from_slice(&(dirs as u64).to_le_bytes());
        toc[16..24].copy_from_slice(&(files as u64).to_le_bytes());
        toc[24..32].copy_from_slice(&(pool_at as u64).to_le_bytes());
        toc[32..40].copy_from_slice(&(names.len() as u64).to_le_bytes());
        let p = 0x30;
        toc[p..p + 8].copy_from_slice(&0x0400_3F40_232D_E000u64.to_le_bytes());
        toc[p + 8..p + 12].copy_from_slice(&0u32.to_le_bytes()); // root
        toc[p + 12..p + 16].copy_from_slice(&1u32.to_le_bytes()); // exefs
        toc[p + 16..p + 20].copy_from_slice(&u32::MAX.to_le_bytes()); // no romfs
        let d = 0x30 + 0x40;
        // root: name "" , child slot 0 (exefs dir); exefs: name "exefs", child slot 1 (file)
        toc[d + 8..d + 12].copy_from_slice(&0u32.to_le_bytes());
        toc[d + 12..d + 16].copy_from_slice(&1u32.to_le_bytes());
        toc[d + 0x18 + 4..d + 0x18 + 8].copy_from_slice(&5u32.to_le_bytes());
        toc[d + 0x18 + 8..d + 0x18 + 12].copy_from_slice(&1u32.to_le_bytes());
        toc[d + 0x18 + 12..d + 0x18 + 16].copy_from_slice(&1u32.to_le_bytes());
        let blob_at = (0x80 + toc_len).div_ceil(16) * 16;
        let f = d + 2 * 0x18;
        toc[f..f + 4].copy_from_slice(&5u32.to_le_bytes());
        toc[f + 4..f + 8].copy_from_slice(&4u32.to_le_bytes());
        toc[f + 8..f + 16].copy_from_slice(&(main.len() as u64).to_le_bytes());
        toc[f + 16..f + 24].copy_from_slice(&(blob_at as u64).to_le_bytes());
        toc[f + 24..f + 32].copy_from_slice(&(main.len() as u64).to_le_bytes());
        toc[f + 32..f + 64].copy_from_slice(&Sha256::digest(&main));
        toc[children_at..children_at + 4].copy_from_slice(&1u32.to_le_bytes()); // root -> dir 1
        toc[children_at + 4..children_at + 8].copy_from_slice(&2u32.to_le_bytes()); // exefs -> file 0 (id 2)
        toc[pool_at..].copy_from_slice(names);
        let total = blob_at + main.len();
        let mut sb = vec![0u8; 0x80];
        sb[0..4].copy_from_slice(b"NUP\x01");
        sb[4..6].copy_from_slice(&0x80u16.to_le_bytes());
        sb[6..8].copy_from_slice(&1u16.to_le_bytes());
        sb[8..16].copy_from_slice(&0x80u64.to_le_bytes());
        sb[16..24].copy_from_slice(&(toc_len as u64).to_le_bytes());
        sb[0x18..0x38].copy_from_slice(&Sha256::digest(&toc));
        sb[0x38..0x40].copy_from_slice(&(blob_at as u64).to_le_bytes());
        sb[0x40..0x48].copy_from_slice(&(total as u64).to_le_bytes());
        sb[0x48..0x4C].copy_from_slice(&1u32.to_le_bytes());
        let mut out = sb;
        out.extend_from_slice(&toc);
        out.resize(blob_at, 0);
        out.extend_from_slice(&main);
        out
    }

    fn synthetic_graph(dirs: &[(u32, u32)], children: &[u32], with_file: bool) -> Vec<u8> {
        let files = usize::from(with_file);
        let files_at = TOC_HEADER + PACKAGE_SIZE + dirs.len() * DIR_SIZE;
        let children_at = files_at + files * FILE_SIZE;
        let pool_at = children_at + children.len() * 4;
        let mut toc = vec![0u8; pool_at + 1];
        toc[0..4].copy_from_slice(b"TOC\x01");
        toc[4..8].copy_from_slice(&1u32.to_le_bytes());
        toc[8..16].copy_from_slice(&(dirs.len() as u64).to_le_bytes());
        toc[16..24].copy_from_slice(&(files as u64).to_le_bytes());
        toc[24..32].copy_from_slice(&(pool_at as u64).to_le_bytes());
        toc[32..40].copy_from_slice(&1u64.to_le_bytes());
        let p = TOC_HEADER;
        toc[p..p + 8].copy_from_slice(&0x0400_3F40_232D_E000u64.to_le_bytes());
        toc[p + 12..p + 16].copy_from_slice(&NONE.to_le_bytes());
        for (i, (first, count)) in dirs.iter().enumerate() {
            let d = TOC_HEADER + PACKAGE_SIZE + i * DIR_SIZE;
            toc[d + 4..d + 8].copy_from_slice(&u32::from(i != 0).to_le_bytes());
            toc[d + 8..d + 12].copy_from_slice(&first.to_le_bytes());
            toc[d + 12..d + 16].copy_from_slice(&count.to_le_bytes());
        }
        let blob_at = (SUPERBLOCK_SIZE + toc.len()).div_ceil(16) * 16;
        if with_file {
            toc[files_at + 4..files_at + 8].copy_from_slice(&1u32.to_le_bytes());
            toc[files_at + 8..files_at + 16].copy_from_slice(&1u64.to_le_bytes());
            toc[files_at + 16..files_at + 24].copy_from_slice(&(blob_at as u64).to_le_bytes());
            toc[files_at + 24..files_at + 32].copy_from_slice(&1u64.to_le_bytes());
            toc[files_at + 32..files_at + 64].copy_from_slice(&Sha256::digest(b"x"));
        }
        for (i, child) in children.iter().enumerate() {
            toc[children_at + i * 4..children_at + i * 4 + 4].copy_from_slice(&child.to_le_bytes());
        }
        toc[pool_at] = b'r';
        let mut sb = vec![0u8; SUPERBLOCK_SIZE];
        sb[0..4].copy_from_slice(b"NUP\x01");
        sb[4..6].copy_from_slice(&(SUPERBLOCK_SIZE as u16).to_le_bytes());
        sb[6..8].copy_from_slice(&1u16.to_le_bytes());
        sb[8..16].copy_from_slice(&(SUPERBLOCK_SIZE as u64).to_le_bytes());
        sb[16..24].copy_from_slice(&(toc.len() as u64).to_le_bytes());
        sb[0x18..0x38].copy_from_slice(&Sha256::digest(&toc));
        sb[0x38..0x40].copy_from_slice(&(blob_at as u64).to_le_bytes());
        sb[0x40..0x48].copy_from_slice(&((blob_at + files) as u64).to_le_bytes());
        sb[0x48..0x4c].copy_from_slice(&1u32.to_le_bytes());
        sb.extend_from_slice(&toc);
        sb.resize(blob_at, 0);
        if with_file {
            sb.push(b'x');
        }
        sb
    }

    fn rehash_toc(bytes: &mut [u8]) {
        let size = u64_at(bytes, 0x10) as usize;
        let hash = Sha256::digest(&bytes[SUPERBLOCK_SIZE..SUPERBLOCK_SIZE + size]);
        bytes[0x18..0x38].copy_from_slice(&hash);
    }

    fn with_synthetic<T>(name: &str, data: &[u8], f: impl FnOnce(&Nup) -> T) -> T {
        let path = std::env::temp_dir().join(format!("aod-nup-{name}-{}.nup", std::process::id()));
        std::fs::write(&path, data).unwrap();
        let nup = Nup::open(&path).unwrap();
        let result = f(&nup);
        drop(nup);
        let _ = std::fs::remove_file(path);
        result
    }

    #[test]
    fn rejects_huge_declared_decompressed_size() {
        let mut bytes = synthetic();
        let package = SUPERBLOCK_SIZE + TOC_HEADER;
        bytes[package + 0x14..package + 0x18].copy_from_slice(&196608u32.to_le_bytes());
        bytes[package + 0x18] = 1;
        bytes[package + 0x19] = 1;
        let file = SUPERBLOCK_SIZE + TOC_HEADER + PACKAGE_SIZE + 2 * DIR_SIZE;
        bytes[file + 8..file + 16].copy_from_slice(&(1u64 << 40).to_le_bytes());
        bytes[file + 0x40] = 1;
        rehash_toc(&mut bytes);
        with_synthetic("huge", &bytes, |nup| {
            let entry = nup.entry(0).unwrap();
            assert!(matches!(nup.read_all(&entry), Err(Error::Corrupt(s)) if s.contains("too large to read whole")));
            let mut profile = profile::GameProfile::hwaoi();
            profile.modules.clear();
            profile.modules.insert("main".into(), hex::encode(entry.sha256));
            assert!(matches!(profile::check(nup, None, &profile), Err(Error::Corrupt(s)) if s.contains("too large to read whole")));
        });
    }

    #[test]
    fn rejects_too_many_packages_and_oversized_metadata() {
        let path = std::env::temp_dir().join(format!("aod-nup-limits-{}.nup", std::process::id()));
        let mut bytes = synthetic();
        bytes[0x48..0x4c].copy_from_slice(&300u32.to_le_bytes());
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(Nup::open(&path), Err(Error::Corrupt(s)) if s == "too many packages"));
        let mut bytes = synthetic();
        let package = SUPERBLOCK_SIZE + TOC_HEADER;
        bytes[package + 0x20..package + 0x28].copy_from_slice(&(SUPERBLOCK_SIZE as u64).to_le_bytes());
        bytes[package + 0x28..package + 0x2c].copy_from_slice(&(100u32 << 20).to_le_bytes());
        rehash_toc(&mut bytes);
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(Nup::open(&path), Err(Error::Corrupt(s)) if s == "package metadata too large"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn folder_indices_and_listings_are_bounded() {
        let data = synthetic_graph(&[(0, 1), (0, 0)], &[1], false);
        with_synthetic("bounds", &data, |nup| {
            assert_eq!(nup.children(0).unwrap().len(), 1);
            assert!(nup.dir_name(1).is_ok());
            for dir in [2, u32::MAX] {
                assert!(matches!(nup.children(dir), Err(Error::Corrupt(s)) if s == "folder index out of bounds"));
                assert!(matches!(nup.dir_name(dir), Err(Error::Corrupt(s)) if s == "folder index out of bounds"));
            }
        });
        // The root lists two slots, and the second one is where the name pool starts: names are not node ids.
        let mut bytes = synthetic_graph(&[(0, 2), (0, 0)], &[1, 1], false);
        let pool_at = u64_at(&bytes, SUPERBLOCK_SIZE + 24);
        bytes[SUPERBLOCK_SIZE + 24..SUPERBLOCK_SIZE + 32].copy_from_slice(&(pool_at - 4).to_le_bytes());
        bytes[SUPERBLOCK_SIZE + 32..SUPERBLOCK_SIZE + 40].copy_from_slice(&5u64.to_le_bytes());
        rehash_toc(&mut bytes);
        with_synthetic("pool", &bytes, |nup| {
            assert!(matches!(nup.children(0), Err(Error::Corrupt(s)) if s == "folder listing out of bounds"));
            assert!(nup.children(1).unwrap().is_empty());
        });
    }

    #[test]
    fn rejects_folder_cycle() {
        let data = synthetic_graph(&[(0, 2), (2, 2)], &[1, 1, 1, 1], false);
        with_synthetic("cycle", &data, |nup| {
            assert!(matches!(nup.walk(0), Err(Error::Corrupt(s)) if s == "folder listed twice"));
        });
    }

    #[test]
    fn rejects_repeated_folder_dag200() {
        let mut children = vec![1; 200];
        children.extend(vec![2; 200]);
        children.extend(vec![3; 200]);
        children.push(4);
        let data = synthetic_graph(&[(0, 200), (200, 200), (400, 200), (600, 1)], &children, true);
        with_synthetic("dag200", &data, |nup| {
            assert!(matches!(nup.walk(0), Err(Error::Corrupt(s)) if s == "folder listed twice"));
        });
    }

    #[test]
    fn rejects_more_file_listings_than_declared() {
        let data = synthetic_graph(&[(0, 2)], &[1, 1], true);
        with_synthetic("files", &data, |nup| {
            assert!(matches!(nup.walk(0), Err(Error::Corrupt(s)) if s == "too many files listed"));
        });
    }

    #[test]
    fn rejects_zstd_output_larger_than_declared() {
        let content = b"more than three bytes";
        let compressed = ruzstd::encoding::compress_to_vec(&content[..], ruzstd::encoding::CompressionLevel::Uncompressed);
        for declared in [0usize, 3] {
            let mut bytes = synthetic();
            let blob_at = u64_at(&bytes, 0x38) as usize;
            bytes.truncate(blob_at);
            bytes.extend_from_slice(&compressed);
            let total = bytes.len() as u64;
            bytes[0x40..0x48].copy_from_slice(&total.to_le_bytes());
            let file = SUPERBLOCK_SIZE + TOC_HEADER + PACKAGE_SIZE + 2 * DIR_SIZE;
            bytes[file + 8..file + 16].copy_from_slice(&(declared as u64).to_le_bytes());
            bytes[file + 24..file + 32].copy_from_slice(&(compressed.len() as u64).to_le_bytes());
            bytes[file + 32..file + 64].copy_from_slice(&Sha256::digest(&content[..declared]));
            bytes[file + 0x40] = 1;
            rehash_toc(&mut bytes);
            with_synthetic(&format!("overflow-{declared}"), &bytes, |nup| {
                let entry = nup.entry(0).unwrap();
                assert!(matches!(nup.read_all(&entry), Err(Error::Corrupt(s)) if s.contains("size mismatch")));
            });
        }
    }

    #[test]
    fn reads_synthetic_nup() {
        let dir = std::env::temp_dir().join(format!("aod-nup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.nup");
        std::fs::write(&path, synthetic()).unwrap();
        let nup = Nup::open(&path).unwrap();
        assert_eq!(nup.packages().len(), 1);
        let p = &nup.packages()[0];
        assert_eq!(p.kind, Kind::Base);
        assert_eq!(p.title_id, 0x0400_3F40_232D_E000);
        let main = nup.lookup(p.exefs_dir.unwrap(), "main").unwrap().unwrap();
        assert_eq!(nup.read_all(&main).unwrap().len(), 0x60);
        assert_eq!(nup.build_id(p).unwrap().as_deref(), Some("0102030405060708090a0b0c0d0e0f1011121314"));
        assert_eq!(nup.newest_program().unwrap().index, 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_damaged_toc() {
        let mut bytes = synthetic();
        bytes[0x80 + 0x30] ^= 0xFF; // flip a byte inside the TOC: its SHA-256 no longer matches
        let path = std::env::temp_dir().join(format!("aod-nup-bad-{}.nup", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        assert!(matches!(Nup::open(&path), Err(Error::Corrupt(_))));
        let _ = std::fs::remove_file(path);
    }
}
