//! Read-only FAT32 card built from a host directory, for the My Lab bootloader.
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const SECTOR: usize = 512;
const PARTITION: u32 = 2048;
const SECTORS: u32 = 4 * 1024 * 1024;
const RESERVED: u32 = 32;
const CLUSTER_SECTORS: u32 = 32;
const CLUSTER: usize = CLUSTER_SECTORS as usize * SECTOR;
const FAT_SECTORS: u32 = 1024;
const DATA: u32 = PARTITION + RESERVED + FAT_SECTORS * 2;
const LIMIT: usize = 128 * 1024 * 1024;

pub struct Card {
    pub directory: PathBuf,
    prefix: Vec<u8>,
    data: Vec<u8>,
    pub read_count: u64,
}

fn word(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn dword(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn short_name(name: &str) -> Option<[u8; 11]> {
    let (base, ext) = name.split_once('.').unwrap_or((name, ""));
    let valid = |part: &str| {
        part.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-$~".contains(&b))
    };
    if base.is_empty() || base.len() > 8 || ext.len() > 3 || !valid(base) || !valid(ext) {
        return None;
    }
    let mut result = [b' '; 11];
    result[..base.len()].copy_from_slice(base.to_ascii_uppercase().as_bytes());
    result[8..8 + ext.len()].copy_from_slice(ext.to_ascii_uppercase().as_bytes());
    Some(result)
}

struct Volume {
    fat: Vec<u32>,
    data: Vec<u8>,
}

impl Volume {
    fn allocate(&mut self, length: usize) -> Result<(u32, usize), String> {
        let offset = self.data.len();
        let clusters = length.div_ceil(CLUSTER);
        let end = offset.saturating_add(clusters.saturating_mul(CLUSTER));
        if end > LIMIT {
            return Err("SD folder exceeds the 128 MiB capacity".into());
        }
        if clusters == 0 {
            return Ok((0, offset));
        }
        let first = self.fat.len() as u32;
        for next in first + 1..first + clusters as u32 {
            self.fat.push(next);
        }
        self.fat.push(0x0fff_ffff);
        self.data.resize(end, 0);
        Ok((first, offset))
    }

    fn directory(&mut self, path: &Path, parent: Option<u32>, depth: usize) -> Result<u32, String> {
        if depth > 16 {
            return Err("SD folders may be nested up to 16 levels".into());
        }
        let mut entries = Vec::new();
        let mut names = BTreeSet::new();
        for item in fs::read_dir(path).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            let name = item.file_name();
            if name.as_encoded_bytes().first() == Some(&b'.') {
                continue;
            }
            let kind = item.file_type().map_err(|e| e.to_string())?;
            if !kind.is_file() && !kind.is_dir() {
                continue;
            }
            let name = name
                .to_str()
                .and_then(short_name)
                .ok_or_else(|| format!("Use an 8.3 FAT name for {}", item.path().display()))?;
            if !names.insert(name) {
                return Err(format!("Duplicate FAT name in {}", path.display()));
            }
            entries.push((name, item, kind));
            if entries.len() > 4096 {
                return Err("SD folders may hold at most 4096 entries".into());
            }
        }
        entries.sort_by_key(|(name, _, _)| *name);
        let special = if parent.is_some() { 2 } else { 0 };
        let (cluster, offset) = self.allocate((entries.len() + special + 1) * 32)?;
        let entry = |out: &mut [u8], name: &[u8; 11], attr, cluster: u32, size| {
            out[..11].copy_from_slice(name);
            out[11] = attr;
            word(out, 20, (cluster >> 16) as u16);
            word(out, 26, cluster as u16);
            dword(out, 28, size);
        };
        if let Some(parent) = parent {
            entry(
                &mut self.data[offset..offset + 32],
                b".          ",
                0x10,
                cluster,
                0,
            );
            entry(
                &mut self.data[offset + 32..offset + 64],
                b"..         ",
                0x10,
                parent,
                0,
            );
        }
        for (index, (name, item, kind)) in entries.into_iter().enumerate() {
            let (child, attr, size) = if kind.is_dir() {
                (
                    self.directory(
                        &item.path(),
                        Some(if parent.is_some() { cluster } else { 0 }),
                        depth + 1,
                    )?,
                    0x10,
                    0,
                )
            } else {
                use std::io::Read;
                let file = fs::File::open(item.path()).map_err(|e| e.to_string())?;
                let length = usize::try_from(file.metadata().map_err(|e| e.to_string())?.len())
                    .map_err(|_| "SD file is too large")?;
                let (child, start) = self.allocate(length)?;
                let mut file = file;
                file.read_exact(&mut self.data[start..start + length])
                    .map_err(|e| e.to_string())?;
                (child, 0x21, length as u32)
            };
            let start = offset + (index + special) * 32;
            entry(&mut self.data[start..start + 32], &name, attr, child, size);
        }
        Ok(cluster)
    }
}

impl Card {
    pub fn open(directory: PathBuf) -> Result<Self, String> {
        let mut volume = Volume {
            fat: vec![0x0fff_fff8, 0x0fff_ffff],
            data: Vec::new(),
        };
        let root = volume.directory(&directory, None, 0)?;
        let mut prefix = vec![0; DATA as usize * SECTOR];
        prefix[450] = 0x0c;
        dword(&mut prefix, 454, PARTITION);
        dword(&mut prefix, 458, SECTORS);
        word(&mut prefix, 510, 0xaa55);
        let offset = PARTITION as usize * SECTOR;
        let mut boot = [0u8; SECTOR];
        boot[..11].copy_from_slice(b"\xeb\x58\x90MSDOS5.0");
        word(&mut boot, 11, SECTOR as u16);
        boot[13] = CLUSTER_SECTORS as u8;
        word(&mut boot, 14, RESERVED as u16);
        boot[16] = 2;
        boot[21] = 0xf8;
        word(&mut boot, 24, 63);
        word(&mut boot, 26, 255);
        dword(&mut boot, 28, PARTITION);
        dword(&mut boot, 32, SECTORS);
        dword(&mut boot, 36, FAT_SECTORS);
        dword(&mut boot, 44, root);
        word(&mut boot, 48, 1);
        word(&mut boot, 50, 6);
        boot[64] = 0x80;
        boot[66] = 0x29;
        boot[71..90].copy_from_slice(b"CAPYBARA SDFAT32   ");
        word(&mut boot, 510, 0xaa55);
        for sector in [offset, offset + 6 * SECTOR] {
            prefix[sector..sector + SECTOR].copy_from_slice(&boot);
            let info = &mut prefix[sector + SECTOR..sector + 2 * SECTOR];
            dword(info, 0, 0x4161_5252);
            dword(info, 484, 0x6141_7272);
            dword(info, 488, u32::MAX);
            dword(info, 492, u32::MAX);
            dword(info, 508, 0xaa55_0000);
        }
        for copy in 0..2 {
            let base = (PARTITION + RESERVED + copy * FAT_SECTORS) as usize * SECTOR;
            for (index, value) in volume.fat.iter().enumerate() {
                dword(&mut prefix, base + index * 4, *value);
            }
        }
        Ok(Self {
            directory,
            prefix,
            data: volume.data,
            read_count: 0,
        })
    }

    pub fn sector_count(&self) -> u32 {
        PARTITION + SECTORS
    }

    pub fn read(&mut self, lba: u32, out: &mut [u8]) -> bool {
        if out.is_empty()
            || out.len() % SECTOR != 0
            || u64::from(lba) + (out.len() / SECTOR) as u64 > u64::from(self.sector_count())
        {
            return false;
        }
        for (index, sector) in out.chunks_exact_mut(SECTOR).enumerate() {
            let offset = (lba as usize + index) * SECTOR;
            let content = if offset < self.prefix.len() {
                self.prefix.get(offset..offset + SECTOR)
            } else {
                let offset = offset - self.prefix.len();
                self.data.get(offset..offset + SECTOR)
            };
            if let Some(content) = content {
                sector.copy_from_slice(content);
            } else {
                sector.fill(0);
            }
        }
        self.read_count += (out.len() / SECTOR) as u64;
        true
    }
}
