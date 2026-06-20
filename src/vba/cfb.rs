use std::collections::{BTreeMap, BTreeSet};

const MAX_REGULAR_SECTOR_CHAIN: usize = 1 << 20;
const MAX_MINI_SECTOR_CHAIN: usize = 1 << 22;

const SECTOR_FREE: u32 = 0xFFFF_FFFF;
const SECTOR_END: u32 = 0xFFFF_FFFE;
const SECTOR_FAT: u32 = 0xFFFF_FFFD;
const SECTOR_DIFAT: u32 = 0xFFFF_FFFC;
const DIRECTORY_STREAM: u8 = 2;
const DIRECTORY_STORAGE: u8 = 1;
const DIRECTORY_ROOT: u8 = 5;

const COMPOUND_SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

pub(super) struct CfbFile<'a> {
    data: &'a [u8],
    sector_size: usize,
    mini_sector_size: usize,
    mini_stream_cutoff: u64,
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    mini_stream: Vec<u8>,
    entries: Vec<DirectoryEntry>,
    streams: BTreeMap<String, usize>,
}

#[derive(Clone, Default)]
struct DirectoryEntry {
    name: String,
    path: String,
    object_type: u8,
    left_sibling: u32,
    right_sibling: u32,
    child: u32,
    start_sector: u32,
    size: u64,
}

impl<'a> CfbFile<'a> {
    pub(super) fn open(data: &'a [u8]) -> Result<Self, String> {
        if data.len() < 512 || data[..8] != COMPOUND_SIGNATURE {
            return Err("not a Compound File Binary vbaProject.bin".to_string());
        }
        if read_u16(data, 28)? != 0xFFFE {
            return Err("unsupported CFB byte order".to_string());
        }

        let sector_shift = read_u16(data, 30)?;
        let mini_sector_shift = read_u16(data, 32)?;
        if !(9..=12).contains(&sector_shift) {
            return Err(format!("unsupported CFB sector size shift {sector_shift}"));
        }
        if mini_sector_shift != 6 {
            return Err(format!(
                "unsupported CFB mini sector size shift {mini_sector_shift}"
            ));
        }

        let mut file = Self {
            data,
            sector_size: 1_usize << sector_shift,
            mini_sector_size: 1_usize << mini_sector_shift,
            mini_stream_cutoff: u64::from(read_u32(data, 56)?),
            fat: Vec::new(),
            mini_fat: Vec::new(),
            mini_stream: Vec::new(),
            entries: Vec::new(),
            streams: BTreeMap::new(),
        };
        if file.mini_stream_cutoff == 0 {
            file.mini_stream_cutoff = 4096;
        }

        let num_fat_sectors = read_u32(data, 44)?;
        let first_directory_sector = read_u32(data, 48)?;
        let first_mini_fat_sector = read_u32(data, 60)?;
        let num_mini_fat_sectors = read_u32(data, 64)?;
        let first_difat_sector = read_u32(data, 68)?;
        let num_difat_sectors = read_u32(data, 72)?;

        let fat_sectors =
            file.collect_fat_sectors(num_fat_sectors, first_difat_sector, num_difat_sectors)?;
        file.read_fat(&fat_sectors)?;
        if first_mini_fat_sector != SECTOR_END && num_mini_fat_sectors > 0 {
            file.read_mini_fat(first_mini_fat_sector, num_mini_fat_sectors)?;
        }

        let directory_data = file
            .read_regular_stream(first_directory_sector, 0)
            .map_err(|err| format!("failed to read CFB directory stream: {err}"))?;
        file.parse_directory(&directory_data)?;
        file.build_paths()?;
        if let Some(root) = file.entries.first()
            && root.start_sector != SECTOR_END
            && root.size > 0
        {
            file.mini_stream = file
                .read_regular_stream(root.start_sector, root.size)
                .map_err(|err| format!("failed to read CFB mini stream: {err}"))?;
        }

        Ok(file)
    }

    pub(super) fn stream(&self, path: &str) -> Result<Vec<u8>, String> {
        let normalized = normalize_path(path);
        let entry_index = self.streams.get(&normalized).copied().or_else(|| {
            self.streams
                .iter()
                .find(|(candidate, _)| candidate.eq_ignore_ascii_case(&normalized))
                .map(|(_, index)| *index)
        });
        let Some(entry_index) = entry_index else {
            return Err(format!("CFB stream {path:?} not found"));
        };
        let entry = &self.entries[entry_index];
        if entry.object_type != DIRECTORY_STREAM {
            return Err(format!("CFB path {path:?} is not a stream"));
        }
        if entry.size == 0 {
            return Ok(Vec::new());
        }
        if entry.size < self.mini_stream_cutoff
            && !self.mini_fat.is_empty()
            && !self.mini_stream.is_empty()
        {
            return self.read_mini_stream(entry.start_sector, entry.size);
        }
        self.read_regular_stream(entry.start_sector, entry.size)
    }

    pub(super) fn streams(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.object_type == DIRECTORY_STREAM && !entry.path.is_empty())
            .map(|entry| entry.path.clone())
            .collect()
    }

    fn collect_fat_sectors(
        &self,
        num_fat_sectors: u32,
        first_difat_sector: u32,
        num_difat_sectors: u32,
    ) -> Result<Vec<u32>, String> {
        let mut sectors = Vec::new();
        let mut offset = 76;
        while offset + 4 <= 512 && (sectors.len() as u32) < num_fat_sectors {
            let sector = read_u32(self.data, offset)?;
            if sector != SECTOR_FREE && sector != SECTOR_END {
                sectors.push(sector);
            }
            offset += 4;
        }

        let mut current = first_difat_sector;
        let mut visited = BTreeSet::new();
        for _ in 0..num_difat_sectors {
            if current == SECTOR_END || (sectors.len() as u32) >= num_fat_sectors {
                break;
            }
            if !visited.insert(current) {
                return Err(format!("CFB DIFAT sector chain cycle at sector {current}"));
            }
            let sector_data = self.sector(current)?;
            let entries_per_difat = self.sector_size / 4 - 1;
            for index in 0..entries_per_difat {
                if (sectors.len() as u32) >= num_fat_sectors {
                    break;
                }
                let sector = read_u32(sector_data, index * 4)?;
                if sector != SECTOR_FREE && sector != SECTOR_END {
                    sectors.push(sector);
                }
            }
            current = read_u32(sector_data, self.sector_size - 4)?;
        }
        if (sectors.len() as u32) < num_fat_sectors {
            return Err(format!(
                "CFB DIFAT listed {} FAT sectors, want {num_fat_sectors}",
                sectors.len()
            ));
        }
        Ok(sectors)
    }

    fn read_fat(&mut self, fat_sectors: &[u32]) -> Result<(), String> {
        for &fat_sector in fat_sectors {
            let sector_data = self
                .sector(fat_sector)
                .map_err(|err| format!("failed to read FAT sector {fat_sector}: {err}"))?;
            let mut offset = 0;
            while offset + 4 <= sector_data.len() {
                self.fat.push(read_u32(sector_data, offset)?);
                offset += 4;
            }
        }
        Ok(())
    }

    fn read_mini_fat(&mut self, first_sector: u32, num_sectors: u32) -> Result<(), String> {
        let mut max_sectors = num_sectors as usize + 1;
        if max_sectors == 0 || max_sectors > MAX_REGULAR_SECTOR_CHAIN {
            max_sectors = MAX_REGULAR_SECTOR_CHAIN;
        }
        let chain = self
            .regular_sector_chain(
                first_sector,
                u64::from(num_sectors) * self.sector_size as u64,
                max_sectors,
            )
            .map_err(|err| format!("failed to read mini FAT chain: {err}"))?;
        for sector_data in chain {
            let mut offset = 0;
            while offset + 4 <= sector_data.len() {
                self.mini_fat.push(read_u32(&sector_data, offset)?);
                offset += 4;
            }
        }
        Ok(())
    }

    fn parse_directory(&mut self, data: &[u8]) -> Result<(), String> {
        if !data.len().is_multiple_of(128) {
            return Err(format!(
                "CFB directory stream size {} is not a multiple of 128",
                data.len()
            ));
        }
        for raw in data.chunks_exact(128) {
            let mut name_len = usize::from(read_u16(raw, 64)?);
            if name_len > 64 {
                name_len = 64;
            }
            let name = if name_len >= 2 {
                decode_utf16_name(&raw[..name_len - 2])
            } else {
                String::new()
            };
            let size = if self.sector_size == 512 {
                u64::from(read_u32(raw, 120)?)
            } else {
                read_u64(raw, 120)?
            };
            self.entries.push(DirectoryEntry {
                name,
                object_type: raw[66],
                left_sibling: read_u32(raw, 68)?,
                right_sibling: read_u32(raw, 72)?,
                child: read_u32(raw, 76)?,
                start_sector: read_u32(raw, 116)?,
                size,
                ..DirectoryEntry::default()
            });
        }
        if self
            .entries
            .first()
            .is_none_or(|entry| entry.object_type != DIRECTORY_ROOT)
        {
            return Err("CFB root directory entry not found".to_string());
        }
        Ok(())
    }

    fn build_paths(&mut self) -> Result<(), String> {
        let root = self
            .entries
            .first()
            .ok_or_else(|| "CFB root directory entry not found".to_string())?;
        let mut visited = BTreeSet::new();
        self.walk_tree(root.child, "", &mut visited)
    }

    fn walk_tree(
        &mut self,
        index: u32,
        parent: &str,
        visited: &mut BTreeSet<u32>,
    ) -> Result<(), String> {
        if index == SECTOR_FREE || index == SECTOR_END {
            return Ok(());
        }
        let entry_index = usize::try_from(index)
            .map_err(|_| format!("CFB directory index {index} out of range"))?;
        if entry_index >= self.entries.len() {
            return Err(format!("CFB directory index {index} out of range"));
        }
        if !visited.insert(index) {
            return Ok(());
        }
        let entry = self.entries[entry_index].clone();
        self.walk_tree(entry.left_sibling, parent, visited)?;
        if !entry.name.is_empty() {
            let path = normalize_path(&format!("{parent}/{}", entry.name));
            self.entries[entry_index].path = path.clone();
            if entry.object_type == DIRECTORY_STREAM {
                self.streams.insert(path.clone(), entry_index);
            }
            if entry.object_type == DIRECTORY_STORAGE {
                self.walk_tree(entry.child, &path, visited)?;
            }
        }
        self.walk_tree(entry.right_sibling, parent, visited)
    }

    fn read_regular_stream(&self, first_sector: u32, size: u64) -> Result<Vec<u8>, String> {
        let chunks = self.regular_sector_chain(first_sector, size, MAX_REGULAR_SECTOR_CHAIN)?;
        let mut data = Vec::new();
        for chunk in chunks {
            data.extend_from_slice(&chunk);
        }
        if size > 0 && data.len() as u64 > size {
            data.truncate(size as usize);
        }
        Ok(data)
    }

    fn regular_sector_chain(
        &self,
        first_sector: u32,
        size: u64,
        max_sectors: usize,
    ) -> Result<Vec<Vec<u8>>, String> {
        if first_sector == SECTOR_END || first_sector == SECTOR_FREE {
            if size == 0 {
                return Ok(Vec::new());
            }
            return Err("stream has no starting sector".to_string());
        }
        let mut chunks = Vec::new();
        let mut current = first_sector;
        while current != SECTOR_END {
            if current == SECTOR_FREE || current == SECTOR_FAT || current == SECTOR_DIFAT {
                return Err(format!(
                    "invalid sector marker 0x{current:08x} in stream chain"
                ));
            }
            let current_index =
                usize::try_from(current).map_err(|_| format!("sector {current} outside FAT"))?;
            if current_index >= self.fat.len() {
                return Err(format!("sector {current} outside FAT"));
            }
            chunks.push(self.sector(current)?.to_vec());
            if chunks.len() > max_sectors {
                return Err("sector chain exceeded safety limit".to_string());
            }
            if size > 0 && (chunks.len() * self.sector_size) as u64 >= size {
                break;
            }
            current = self.fat[current_index];
        }
        Ok(chunks)
    }

    fn read_mini_stream(&self, first_mini_sector: u32, size: u64) -> Result<Vec<u8>, String> {
        if first_mini_sector == SECTOR_END || first_mini_sector == SECTOR_FREE {
            if size == 0 {
                return Ok(Vec::new());
            }
            return Err("mini stream has no starting sector".to_string());
        }
        let mut out = Vec::new();
        let mut current = first_mini_sector;
        while current != SECTOR_END {
            let current_index = usize::try_from(current)
                .map_err(|_| format!("mini sector {current} outside mini FAT"))?;
            if current_index >= self.mini_fat.len() {
                return Err(format!("mini sector {current} outside mini FAT"));
            }
            let start = current_index
                .checked_mul(self.mini_sector_size)
                .ok_or_else(|| format!("mini sector {current} outside mini stream"))?;
            let end = start
                .checked_add(self.mini_sector_size)
                .ok_or_else(|| format!("mini sector {current} outside mini stream"))?;
            if end > self.mini_stream.len() {
                return Err(format!("mini sector {current} outside mini stream"));
            }
            out.extend_from_slice(&self.mini_stream[start..end]);
            if out.len() > MAX_MINI_SECTOR_CHAIN * self.mini_sector_size {
                return Err("mini sector chain exceeded safety limit".to_string());
            }
            if out.len() as u64 >= size {
                break;
            }
            current = self.mini_fat[current_index];
        }
        if out.len() as u64 > size {
            out.truncate(size as usize);
        }
        Ok(out)
    }

    fn sector(&self, index: u32) -> Result<&'a [u8], String> {
        let index = usize::try_from(index).map_err(|_| format!("sector {index} outside file"))?;
        let start = 512_usize
            .checked_add(
                index
                    .checked_mul(self.sector_size)
                    .ok_or_else(|| format!("sector {index} outside file"))?,
            )
            .ok_or_else(|| format!("sector {index} outside file"))?;
        let end = start
            .checked_add(self.sector_size)
            .ok_or_else(|| format!("sector {index} outside file"))?;
        if end > self.data.len() {
            return Err(format!("sector {index} outside file"));
        }
        Ok(&self.data[start..end])
    }
}

fn normalize_path(path: &str) -> String {
    let path = path.trim().replace('\\', "/");
    let path = path.trim_matches('/');
    path.split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| "truncated CFB structure".to_string())?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| "truncated CFB structure".to_string())?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u64(data: &[u8], offset: usize) -> Result<u64, String> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or_else(|| "truncated CFB structure".to_string())?;
    Ok(u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn decode_utf16_name(data: &[u8]) -> String {
    let mut units = Vec::with_capacity(data.len() / 2);
    for chunk in data.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        if value == 0 {
            break;
        }
        units.push(value);
    }
    String::from_utf16_lossy(&units)
}
