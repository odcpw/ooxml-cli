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
const WRITER_SECTOR_SIZE: usize = 512;
const WRITER_SECTOR_SHIFT: u16 = 9;
const WRITER_MINI_SECTOR_SHIFT: u16 = 6;
const WRITER_MINI_SECTOR_SIZE: usize = 64;
const WRITER_MINI_STREAM_CUTOFF: usize = 4096;
const WRITER_MAX_DIFAT_ENTRIES: usize = 109;

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
    color: u8,
    left_sibling: u32,
    right_sibling: u32,
    child: u32,
    clsid: [u8; 16],
    state_bits: u32,
    creation_time: [u8; 8],
    modified_time: [u8; 8],
    start_sector: u32,
    size: u64,
}

#[derive(Clone)]
struct WriteDirectoryEntry {
    name: String,
    path: String,
    object_type: u8,
    color: u8,
    left_sibling: u32,
    right_sibling: u32,
    child: u32,
    clsid: [u8; 16],
    state_bits: u32,
    creation_time: [u8; 8],
    modified_time: [u8; 8],
    start_sector: u32,
    size: u64,
    parent: String,
}

type DirectoryBuild = (
    Vec<WriteDirectoryEntry>,
    BTreeMap<usize, Vec<usize>>,
    Vec<usize>,
    bool,
);

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
                color: raw[67],
                left_sibling: read_u32(raw, 68)?,
                right_sibling: read_u32(raw, 72)?,
                child: read_u32(raw, 76)?,
                clsid: raw[80..96].try_into().unwrap_or_default(),
                state_bits: read_u32(raw, 96)?,
                creation_time: raw[100..108].try_into().unwrap_or_default(),
                modified_time: raw[108..116].try_into().unwrap_or_default(),
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

pub(super) fn rewrite_streams_with_deletes(
    data: &[u8],
    replacements: &BTreeMap<String, Vec<u8>>,
    deletes: &[String],
) -> Result<Vec<u8>, String> {
    rewrite_streams_with_adds_and_deletes(data, replacements, &BTreeMap::new(), deletes)
}

pub(super) fn rewrite_streams_with_adds_and_deletes(
    data: &[u8],
    replacements: &BTreeMap<String, Vec<u8>>,
    additions: &BTreeMap<String, Vec<u8>>,
    deletes: &[String],
) -> Result<Vec<u8>, String> {
    let file = CfbFile::open(data)?;
    let delete_set = deletes
        .iter()
        .map(|path| normalize_path(path))
        .collect::<BTreeSet<_>>();
    let mut streams = BTreeMap::<String, Vec<u8>>::new();
    for path in file.streams() {
        let normalized = normalize_path(&path);
        if delete_set.contains(&normalized) {
            continue;
        }
        streams.insert(normalized, file.stream(&path)?);
    }
    for (path, stream_data) in replacements {
        let normalized = normalize_path(path);
        if !streams.contains_key(&normalized) {
            return Err(format!("CFB stream {path:?} not found"));
        }
        streams.insert(normalized, stream_data.clone());
    }
    for (path, stream_data) in additions {
        let normalized = normalize_path(path);
        if streams.contains_key(&normalized) {
            return Err(format!("CFB stream {path:?} already exists"));
        }
        streams.insert(normalized, stream_data.clone());
    }
    build_regular_sector_file(&streams, Some(&file))
}

fn build_regular_sector_file(
    streams: &BTreeMap<String, Vec<u8>>,
    source: Option<&CfbFile<'_>>,
) -> Result<Vec<u8>, String> {
    if streams.is_empty() {
        return Err("cannot build CFB file with no streams".to_string());
    }
    let (mut entries, children, stream_order, preserve_tree) =
        build_directory_entries(streams, source)?;

    let mut regular_stream_sectors = BTreeMap::<usize, Vec<u32>>::new();
    let mut regular_sectors = Vec::<Vec<u8>>::new();
    let mut mini_chains = BTreeMap::<usize, Vec<u32>>::new();
    let mut mini_fat = Vec::<u32>::new();
    let mut mini_stream = Vec::<u8>::new();

    for entry_index in stream_order {
        let entry = &mut entries[entry_index];
        let data = streams
            .get(&entry.path)
            .ok_or_else(|| format!("CFB stream data missing for {}", entry.path))?;
        entry.size = data.len() as u64;
        if data.is_empty() {
            entry.start_sector = SECTOR_END;
            continue;
        }
        if data.len() < WRITER_MINI_STREAM_CUTOFF {
            let mut padded = data.clone();
            while !padded.len().is_multiple_of(WRITER_MINI_SECTOR_SIZE) {
                padded.push(0);
            }
            entry.start_sector = mini_fat.len() as u32;
            while !padded.is_empty() {
                mini_stream.extend_from_slice(&padded[..WRITER_MINI_SECTOR_SIZE]);
                mini_chains
                    .entry(entry_index)
                    .or_default()
                    .push(mini_fat.len() as u32);
                mini_fat.push(SECTOR_FREE);
                padded.drain(..WRITER_MINI_SECTOR_SIZE);
            }
            continue;
        }
        let mut padded = data.clone();
        while !padded.len().is_multiple_of(WRITER_SECTOR_SIZE) {
            padded.push(0);
        }
        let start = regular_sectors.len() as u32;
        while !padded.is_empty() {
            regular_sectors.push(padded[..WRITER_SECTOR_SIZE].to_vec());
            let len = regular_stream_sectors
                .get(&entry_index)
                .map(Vec::len)
                .unwrap_or_default() as u32;
            regular_stream_sectors
                .entry(entry_index)
                .or_default()
                .push(start + len);
            padded.drain(..WRITER_SECTOR_SIZE);
        }
    }

    for chain in mini_chains.values() {
        link_chain(&mut mini_fat, chain);
    }

    let mut mini_stream_sectors = Vec::<u32>::new();
    if !mini_stream.is_empty() {
        entries[0].size = mini_stream.len() as u64;
        let mut padded = mini_stream;
        while !padded.len().is_multiple_of(WRITER_SECTOR_SIZE) {
            padded.push(0);
        }
        let start = regular_sectors.len() as u32;
        while !padded.is_empty() {
            regular_sectors.push(padded[..WRITER_SECTOR_SIZE].to_vec());
            mini_stream_sectors.push(start + mini_stream_sectors.len() as u32);
            padded.drain(..WRITER_SECTOR_SIZE);
        }
    } else {
        entries[0].start_sector = SECTOR_END;
        entries[0].size = 0;
    }

    if !preserve_tree {
        for (parent_index, child_indexes) in children {
            if !child_indexes.is_empty() {
                entries[parent_index].child =
                    assign_directory_sibling_tree(&mut entries, &child_indexes) as u32;
            }
        }
    }

    let directory_sector_count = sectors_needed(entries.len() * 128, WRITER_SECTOR_SIZE);
    let mini_fat_sector_count = sectors_needed(mini_fat.len() * 4, WRITER_SECTOR_SIZE);
    let data_sector_count = regular_sectors.len() + mini_fat_sector_count + directory_sector_count;
    let mut fat_sector_count = 1;
    loop {
        let next = sectors_needed(data_sector_count + fat_sector_count, WRITER_SECTOR_SIZE / 4);
        if next == fat_sector_count {
            break;
        }
        fat_sector_count = next;
    }
    if fat_sector_count > WRITER_MAX_DIFAT_ENTRIES {
        return Err(format!(
            "CFB file needs {fat_sector_count} FAT sectors; writer supports at most {WRITER_MAX_DIFAT_ENTRIES}"
        ));
    }

    let sector_base = fat_sector_count as u32;
    for (entry_index, sectors) in &regular_stream_sectors {
        entries[*entry_index].start_sector = sector_base + sectors[0];
    }
    if !mini_stream_sectors.is_empty() {
        entries[0].start_sector = sector_base + mini_stream_sectors[0];
    }
    let mini_fat_start = if mini_fat_sector_count > 0 {
        sector_base + regular_sectors.len() as u32
    } else {
        SECTOR_END
    };
    let directory_start = sector_base + (regular_sectors.len() + mini_fat_sector_count) as u32;

    for sectors in regular_stream_sectors.values_mut() {
        for sector in sectors {
            *sector += sector_base;
        }
    }
    for sector in &mut mini_stream_sectors {
        *sector += sector_base;
    }

    let mut directory_data = serialize_directory(&entries);
    let mut directory_sectors = Vec::<Vec<u8>>::new();
    while !directory_data.is_empty() {
        directory_sectors.push(directory_data[..WRITER_SECTOR_SIZE].to_vec());
        directory_data.drain(..WRITER_SECTOR_SIZE);
    }

    let mut mini_fat_sectors = Vec::<Vec<u8>>::new();
    if mini_fat_sector_count > 0 {
        let mut mini_fat_data = vec![0_u8; mini_fat_sector_count * WRITER_SECTOR_SIZE];
        for (idx, value) in mini_fat.iter().enumerate() {
            mini_fat_data[idx * 4..idx * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        for idx in mini_fat.len()..mini_fat_data.len() / 4 {
            mini_fat_data[idx * 4..idx * 4 + 4].copy_from_slice(&SECTOR_FREE.to_le_bytes());
        }
        while !mini_fat_data.is_empty() {
            mini_fat_sectors.push(mini_fat_data[..WRITER_SECTOR_SIZE].to_vec());
            mini_fat_data.drain(..WRITER_SECTOR_SIZE);
        }
    }

    let total_sectors = fat_sector_count + data_sector_count;
    let mut fat = vec![SECTOR_FREE; total_sectors];
    for item in fat.iter_mut().take(fat_sector_count) {
        *item = SECTOR_FAT;
    }
    for sectors in regular_stream_sectors.values() {
        link_chain(&mut fat, sectors);
    }
    link_chain(&mut fat, &mini_stream_sectors);
    if mini_fat_sector_count > 0 {
        let mini_fat_chain = (0..mini_fat_sector_count)
            .map(|idx| mini_fat_start + idx as u32)
            .collect::<Vec<_>>();
        link_chain(&mut fat, &mini_fat_chain);
    }
    let directory_chain = (0..directory_sectors.len())
        .map(|idx| directory_start + idx as u32)
        .collect::<Vec<_>>();
    link_chain(&mut fat, &directory_chain);

    let mut out = build_header(
        fat_sector_count as u32,
        directory_start,
        mini_fat_start,
        mini_fat_sector_count as u32,
    );
    for fat_index in 0..fat_sector_count {
        let mut sector = vec![0_u8; WRITER_SECTOR_SIZE];
        let start = fat_index * WRITER_SECTOR_SIZE / 4;
        for idx in 0..WRITER_SECTOR_SIZE / 4 {
            let value = fat.get(start + idx).copied().unwrap_or(SECTOR_FREE);
            sector[idx * 4..idx * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        out.extend(sector);
    }
    for sector in regular_sectors
        .into_iter()
        .chain(mini_fat_sectors)
        .chain(directory_sectors)
    {
        out.extend(sector);
    }
    Ok(out)
}

fn build_directory_entries(
    streams: &BTreeMap<String, Vec<u8>>,
    source: Option<&CfbFile<'_>>,
) -> Result<DirectoryBuild, String> {
    if let Some(source) = source {
        build_directory_entries_from_source(streams, source)
    } else {
        build_directory_entries_sorted(streams)
    }
}

fn build_directory_entries_sorted(
    streams: &BTreeMap<String, Vec<u8>>,
) -> Result<DirectoryBuild, String> {
    let mut entries = vec![WriteDirectoryEntry {
        name: "Root Entry".to_string(),
        object_type: DIRECTORY_ROOT,
        color: 1,
        left_sibling: SECTOR_FREE,
        right_sibling: SECTOR_FREE,
        child: SECTOR_FREE,
        start_sector: SECTOR_END,
        path: String::new(),
        parent: String::new(),
        clsid: [0; 16],
        state_bits: 0,
        creation_time: [0; 8],
        modified_time: [0; 8],
        size: 0,
    }];
    let mut path_to_index = BTreeMap::<String, usize>::new();
    path_to_index.insert(String::new(), 0);
    let mut parent_children = BTreeMap::<String, Vec<usize>>::new();
    let mut paths = streams
        .keys()
        .map(|path| normalize_path(path))
        .collect::<Vec<_>>();
    paths.sort_by(|a, b| directory_path_compare(a, b));
    for path in paths {
        if path.is_empty() {
            return Err("CFB stream path cannot be empty".to_string());
        }
        let parts = path.split('/').collect::<Vec<_>>();
        let mut parent = String::new();
        for index in 0..parts.len() - 1 {
            let storage_path = parts[..=index].join("/");
            if path_to_index.contains_key(&storage_path) {
                parent = storage_path;
                continue;
            }
            validate_directory_name(parts[index])?;
            let entry_index = entries.len();
            entries.push(WriteDirectoryEntry {
                name: parts[index].to_string(),
                path: storage_path.clone(),
                object_type: DIRECTORY_STORAGE,
                color: 1,
                left_sibling: SECTOR_FREE,
                right_sibling: SECTOR_FREE,
                child: SECTOR_FREE,
                start_sector: SECTOR_END,
                parent: parent.clone(),
                clsid: [0; 16],
                state_bits: 0,
                creation_time: [0; 8],
                modified_time: [0; 8],
                size: 0,
            });
            path_to_index.insert(storage_path.clone(), entry_index);
            parent_children
                .entry(parent.clone())
                .or_default()
                .push(entry_index);
            parent = storage_path;
        }
        let name = parts[parts.len() - 1];
        validate_directory_name(name)?;
        let entry_index = entries.len();
        entries.push(WriteDirectoryEntry {
            name: name.to_string(),
            path: path.clone(),
            object_type: DIRECTORY_STREAM,
            color: 1,
            left_sibling: SECTOR_FREE,
            right_sibling: SECTOR_FREE,
            child: SECTOR_FREE,
            start_sector: SECTOR_END,
            parent: parent.clone(),
            clsid: [0; 16],
            state_bits: 0,
            creation_time: [0; 8],
            modified_time: [0; 8],
            size: 0,
        });
        path_to_index.insert(path, entry_index);
        parent_children.entry(parent).or_default().push(entry_index);
    }
    let mut children_by_index = BTreeMap::<usize, Vec<usize>>::new();
    for (parent_path, mut child_indexes) in parent_children {
        child_indexes.sort_by(|a, b| {
            entries[*a]
                .name
                .to_ascii_lowercase()
                .cmp(&entries[*b].name.to_ascii_lowercase())
        });
        let parent_index = path_to_index[&parent_path];
        children_by_index.insert(parent_index, child_indexes);
    }
    let mut stream_order = entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| (entry.object_type == DIRECTORY_STREAM).then_some(idx))
        .collect::<Vec<_>>();
    stream_order.sort_by(|a, b| directory_path_compare(&entries[*a].path, &entries[*b].path));
    Ok((entries, children_by_index, stream_order, false))
}

fn build_directory_entries_from_source(
    streams: &BTreeMap<String, Vec<u8>>,
    source: &CfbFile<'_>,
) -> Result<DirectoryBuild, String> {
    let mut needed_storages = BTreeSet::<String>::new();
    for path in streams.keys() {
        let parts = normalize_path(path)
            .split('/')
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        for index in 0..parts.len().saturating_sub(1) {
            needed_storages.insert(parts[..=index].join("/"));
        }
    }

    let mut entries = Vec::<WriteDirectoryEntry>::with_capacity(source.entries.len());
    let mut path_to_index = BTreeMap::<String, usize>::new();
    path_to_index.insert(String::new(), 0);
    let mut parent_children = BTreeMap::<String, Vec<usize>>::new();
    let mut source_streams = BTreeSet::<String>::new();
    let mut included_stream_count = 0_usize;

    for source_entry in &source.entries {
        let mut entry = WriteDirectoryEntry {
            name: source_entry.name.clone(),
            path: source_entry.path.clone(),
            object_type: source_entry.object_type,
            color: source_entry.color,
            left_sibling: source_entry.left_sibling,
            right_sibling: source_entry.right_sibling,
            child: source_entry.child,
            clsid: source_entry.clsid,
            state_bits: source_entry.state_bits,
            creation_time: source_entry.creation_time,
            modified_time: source_entry.modified_time,
            start_sector: SECTOR_END,
            size: 0,
            parent: parent_path(&source_entry.path),
        };
        let include = match source_entry.object_type {
            DIRECTORY_ROOT => {
                entry.path.clear();
                entry.parent.clear();
                true
            }
            DIRECTORY_STORAGE => needed_storages.contains(&source_entry.path),
            DIRECTORY_STREAM => {
                source_streams.insert(source_entry.path.clone());
                let include = streams.contains_key(&source_entry.path);
                if include {
                    included_stream_count += 1;
                }
                include
            }
            _ => false,
        };
        if !include {
            entries.push(WriteDirectoryEntry {
                name: String::new(),
                path: String::new(),
                object_type: 0,
                color: 0,
                left_sibling: SECTOR_FREE,
                right_sibling: SECTOR_FREE,
                child: SECTOR_FREE,
                clsid: [0; 16],
                state_bits: 0,
                creation_time: [0; 8],
                modified_time: [0; 8],
                start_sector: SECTOR_END,
                size: 0,
                parent: String::new(),
            });
            continue;
        }
        if entry.color != 0 && entry.color != 1 {
            entry.color = 1;
        }
        let idx = entries.len();
        path_to_index.insert(entry.path.clone(), idx);
        if entry.object_type != DIRECTORY_ROOT {
            parent_children
                .entry(entry.parent.clone())
                .or_default()
                .push(idx);
        }
        entries.push(entry);
    }

    let mut added_paths = streams
        .keys()
        .filter(|path| !source_streams.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    added_paths.sort_by(|a, b| directory_path_compare(a, b));
    for path in added_paths.iter() {
        let parts = path.split('/').collect::<Vec<_>>();
        let mut parent = String::new();
        for index in 0..parts.len() - 1 {
            let storage_path = parts[..=index].join("/");
            if path_to_index.contains_key(&storage_path) {
                parent = storage_path;
                continue;
            }
            validate_directory_name(parts[index])?;
            let entry_index = entries.len();
            entries.push(WriteDirectoryEntry {
                name: parts[index].to_string(),
                path: storage_path.clone(),
                object_type: DIRECTORY_STORAGE,
                color: 1,
                left_sibling: SECTOR_FREE,
                right_sibling: SECTOR_FREE,
                child: SECTOR_FREE,
                start_sector: SECTOR_END,
                parent: parent.clone(),
                clsid: [0; 16],
                state_bits: 0,
                creation_time: [0; 8],
                modified_time: [0; 8],
                size: 0,
            });
            path_to_index.insert(storage_path.clone(), entry_index);
            parent_children
                .entry(parent.clone())
                .or_default()
                .push(entry_index);
            parent = storage_path;
        }
        let name = parts[parts.len() - 1];
        validate_directory_name(name)?;
        let entry_index = entries.len();
        entries.push(WriteDirectoryEntry {
            name: name.to_string(),
            path: path.clone(),
            object_type: DIRECTORY_STREAM,
            color: 1,
            left_sibling: SECTOR_FREE,
            right_sibling: SECTOR_FREE,
            child: SECTOR_FREE,
            start_sector: SECTOR_END,
            parent: parent.clone(),
            clsid: [0; 16],
            state_bits: 0,
            creation_time: [0; 8],
            modified_time: [0; 8],
            size: 0,
        });
        path_to_index.insert(path.clone(), entry_index);
        parent_children.entry(parent).or_default().push(entry_index);
    }

    let mut children_by_index = BTreeMap::<usize, Vec<usize>>::new();
    for (parent_path, mut child_indexes) in parent_children {
        child_indexes.sort_by(|a, b| directory_name_compare(&entries[*a].name, &entries[*b].name));
        let parent_index = path_to_index
            .get(&parent_path)
            .copied()
            .ok_or_else(|| format!("CFB storage {parent_path:?} missing for child entries"))?;
        children_by_index.insert(parent_index, child_indexes);
    }

    let stream_order = entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            (entry.object_type == DIRECTORY_STREAM && !entry.path.is_empty()).then_some(idx)
        })
        .collect::<Vec<_>>();
    let preserve_tree = added_paths.is_empty() && included_stream_count == source.streams.len();
    if preserve_tree {
        children_by_index.clear();
    }
    Ok((entries, children_by_index, stream_order, preserve_tree))
}

fn parent_path(path: &str) -> String {
    let path = normalize_path(path);
    path.rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

fn directory_name_compare(a: &str, b: &str) -> std::cmp::Ordering {
    let a_units = a.encode_utf16().count();
    let b_units = b.encode_utf16().count();
    if a_units != b_units {
        return a_units.cmp(&b_units);
    }
    let a_fold = a.to_ascii_uppercase();
    let b_fold = b.to_ascii_uppercase();
    if a_fold != b_fold {
        return a_fold.cmp(&b_fold);
    }
    a.cmp(b)
}

fn directory_path_compare(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts = normalize_path(a)
        .split('/')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let b_parts = normalize_path(b)
        .split('/')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    for (a_part, b_part) in a_parts.iter().zip(&b_parts) {
        if a_part.eq_ignore_ascii_case(b_part) {
            continue;
        }
        return directory_name_compare(a_part, b_part);
    }
    a_parts.len().cmp(&b_parts.len())
}

fn assign_directory_sibling_tree(
    entries: &mut [WriteDirectoryEntry],
    child_indexes: &[usize],
) -> usize {
    if child_indexes.is_empty() {
        return SECTOR_FREE as usize;
    }
    let mid = child_indexes.len() / 2;
    let root = child_indexes[mid];
    let left = assign_directory_sibling_tree(entries, &child_indexes[..mid]);
    if left != SECTOR_FREE as usize {
        entries[root].left_sibling = left as u32;
    }
    let right = assign_directory_sibling_tree(entries, &child_indexes[mid + 1..]);
    if right != SECTOR_FREE as usize {
        entries[root].right_sibling = right as u32;
    }
    root
}

fn validate_directory_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("CFB directory name cannot be empty".to_string());
    }
    if name.encode_utf16().count() > 31 {
        return Err(format!(
            "CFB directory name {name:?} is longer than 31 UTF-16 code units"
        ));
    }
    Ok(())
}

fn serialize_directory(entries: &[WriteDirectoryEntry]) -> Vec<u8> {
    let size = sectors_needed(entries.len() * 128, WRITER_SECTOR_SIZE) * WRITER_SECTOR_SIZE;
    let mut out = Vec::with_capacity(size);
    for entry in entries {
        out.extend(serialize_directory_entry(entry));
    }
    while !out.len().is_multiple_of(WRITER_SECTOR_SIZE) {
        out.push(0);
    }
    out
}

fn serialize_directory_entry(entry: &WriteDirectoryEntry) -> Vec<u8> {
    let mut out = vec![0_u8; 128];
    if entry.object_type == 0 && entry.name.is_empty() {
        return out;
    }
    let name_bytes = utf16_name_bytes(&entry.name);
    out[..name_bytes.len()].copy_from_slice(&name_bytes);
    out[64..66].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    out[66] = entry.object_type;
    out[67] = entry.color;
    out[68..72].copy_from_slice(&entry.left_sibling.to_le_bytes());
    out[72..76].copy_from_slice(&entry.right_sibling.to_le_bytes());
    out[76..80].copy_from_slice(&entry.child.to_le_bytes());
    out[80..96].copy_from_slice(&entry.clsid);
    out[96..100].copy_from_slice(&entry.state_bits.to_le_bytes());
    out[100..108].copy_from_slice(&entry.creation_time);
    out[108..116].copy_from_slice(&entry.modified_time);
    out[116..120].copy_from_slice(&entry.start_sector.to_le_bytes());
    out[120..128].copy_from_slice(&entry.size.to_le_bytes());
    out
}

fn utf16_name_bytes(name: &str) -> Vec<u8> {
    (name.to_string() + "\0")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn build_header(
    num_fat_sectors: u32,
    first_directory_sector: u32,
    first_mini_fat_sector: u32,
    num_mini_fat_sectors: u32,
) -> Vec<u8> {
    let mut header = vec![0_u8; 512];
    header[..8].copy_from_slice(&COMPOUND_SIGNATURE);
    header[24..26].copy_from_slice(&0x003E_u16.to_le_bytes());
    header[26..28].copy_from_slice(&0x0003_u16.to_le_bytes());
    header[28..30].copy_from_slice(&0xFFFE_u16.to_le_bytes());
    header[30..32].copy_from_slice(&WRITER_SECTOR_SHIFT.to_le_bytes());
    header[32..34].copy_from_slice(&WRITER_MINI_SECTOR_SHIFT.to_le_bytes());
    header[44..48].copy_from_slice(&num_fat_sectors.to_le_bytes());
    header[48..52].copy_from_slice(&first_directory_sector.to_le_bytes());
    header[56..60].copy_from_slice(&(WRITER_MINI_STREAM_CUTOFF as u32).to_le_bytes());
    header[60..64].copy_from_slice(&first_mini_fat_sector.to_le_bytes());
    header[64..68].copy_from_slice(&num_mini_fat_sectors.to_le_bytes());
    header[68..72].copy_from_slice(&SECTOR_END.to_le_bytes());
    header[72..76].copy_from_slice(&0_u32.to_le_bytes());
    for idx in 0..num_fat_sectors as usize {
        let offset = 76 + idx * 4;
        header[offset..offset + 4].copy_from_slice(&(idx as u32).to_le_bytes());
    }
    for offset in (76 + num_fat_sectors as usize * 4..512).step_by(4) {
        header[offset..offset + 4].copy_from_slice(&SECTOR_FREE.to_le_bytes());
    }
    header
}

fn link_chain(fat: &mut [u32], sectors: &[u32]) {
    for (idx, sector) in sectors.iter().enumerate() {
        let Ok(sector_index) = usize::try_from(*sector) else {
            continue;
        };
        if sector_index >= fat.len() {
            continue;
        }
        fat[sector_index] = sectors.get(idx + 1).copied().unwrap_or(SECTOR_END);
    }
}

fn sectors_needed(size: usize, sector_size: usize) -> usize {
    if size == 0 {
        0
    } else {
        size.div_ceil(sector_size)
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
