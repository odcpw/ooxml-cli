use crate::{CliError, CliResult};

use super::SourceModule;
use super::codec::{decode_mbcs, decode_utf16_le, read_u16, read_u32, utf16le_bytes};

#[derive(Clone, Default)]
pub(super) struct DirModule {
    pub(super) name: String,
    pub(super) stream_name: String,
    pub(super) kind: String,
    pub(super) source_offset: u32,
}

struct ProjectModulesRecord {
    record_start: usize,
    count_payload: usize,
    count: usize,
    modules_start: usize,
    modules_end: usize,
}

struct DirReader<'a> {
    data: &'a [u8],
    pos: usize,
    code_page: i32,
    modules: Vec<DirModule>,
    warnings: Vec<String>,
}
pub(super) fn rewrite_dir_module_offset(
    data: &[u8],
    target: &SourceModule,
    offset: u32,
) -> CliResult<(Vec<u8>, usize)> {
    let mut out = data.to_vec();
    let record = find_project_modules_record(&out).map_err(CliError::unexpected)?;
    let mut pos = record.modules_start;
    let mut patched = 0;
    for _ in 0..record.count {
        let mut current_module = DirModule::default();
        while out.len().saturating_sub(pos) >= 2 {
            let record_id = read_u16(&out, pos).map_err(CliError::unexpected)?;
            if record_id == 0x002B {
                if out.len().saturating_sub(pos) < 6 {
                    return Err(CliError::unexpected("module terminator is truncated"));
                }
                pos += 6;
                break;
            }
            if out.len().saturating_sub(pos) < 6 {
                return Err(CliError::unexpected(format!(
                    "module record 0x{record_id:04x} is truncated"
                )));
            }
            let record_size = read_u32(&out, pos + 2).map_err(CliError::unexpected)? as usize;
            let payload_start = pos + 6;
            let payload_end = payload_start + record_size;
            if payload_end > out.len() {
                return Err(CliError::unexpected(format!(
                    "module record 0x{record_id:04x} exceeds dir stream size"
                )));
            }
            if record_id == 0x0031 && dir_module_matches_source_module(&current_module, target) {
                if record_size < 4 {
                    return Err(CliError::unexpected("MODULEOFFSET record is too short"));
                }
                out[payload_start..payload_start + 4].copy_from_slice(&offset.to_le_bytes());
                patched += 1;
            }
            let payload = &out[payload_start..payload_end];
            match record_id {
                0x0019 => current_module.name = decode_mbcs(payload, 1252),
                0x0047 => {
                    let name = decode_utf16_le(payload);
                    if !name.is_empty() {
                        current_module.name = name;
                    }
                }
                0x001A => current_module.stream_name = decode_mbcs(payload, 1252),
                0x0032 => {
                    let name = decode_utf16_le(payload);
                    if !name.is_empty() {
                        current_module.stream_name = name;
                    }
                }
                _ => {}
            }
            pos = payload_end;
        }
    }
    Ok((out, patched))
}

pub(super) fn remove_dir_module(data: &[u8], module: &SourceModule) -> CliResult<Vec<u8>> {
    let record = find_project_modules_record(data).map_err(CliError::unexpected)?;
    if record.count <= 1 {
        return Err(CliError::invalid_args(
            "refusing to remove the last VBA module",
        ));
    }
    let mut scan = record.modules_start;
    let mut remove_range = None;
    for _ in 0..record.count {
        let block_start = scan;
        let (dir_module, block_end) =
            read_dir_module_block(data, scan).map_err(CliError::unexpected)?;
        if dir_module_matches_source_module(&dir_module, module) {
            remove_range = Some((block_start, block_end));
            break;
        }
        scan = block_end;
    }
    let Some((remove_start, remove_end)) = remove_range else {
        return Err(CliError::unexpected(format!(
            "VBA module {} was not found in PROJECTMODULES records",
            module.primary_selector
        )));
    };
    let mut out = Vec::with_capacity(data.len() - (remove_end - remove_start));
    out.extend_from_slice(&data[..remove_start]);
    out.extend_from_slice(&data[remove_end..]);
    out[record.count_payload..record.count_payload + 2]
        .copy_from_slice(&((record.count - 1) as u16).to_le_bytes());
    Ok(out)
}

pub(super) fn add_dir_module(data: &[u8], module: &SourceModule) -> CliResult<Vec<u8>> {
    let record = find_project_modules_record(data).map_err(CliError::unexpected)?;
    let module_block = build_dir_module_block(module);
    let mut out = Vec::with_capacity(data.len() + module_block.len());
    out.extend_from_slice(&data[..record.modules_end]);
    out.extend_from_slice(&module_block);
    out.extend_from_slice(&data[record.modules_end..]);
    out[record.count_payload..record.count_payload + 2]
        .copy_from_slice(&((record.count + 1) as u16).to_le_bytes());
    Ok(out)
}

fn build_dir_module_block(module: &SourceModule) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(vba_dir_record(0x0019, module.name.as_bytes()));
    out.extend(vba_dir_record(0x0047, &utf16le_bytes(&module.name)));
    out.extend(vba_dir_record(0x001A, module.stream_name.as_bytes()));
    out.extend(vba_dir_record(0x0032, &utf16le_bytes(&module.stream_name)));
    out.extend(vba_dir_record(0x001C, &[]));
    out.extend(vba_dir_record(0x0048, &[]));
    out.extend(vba_dir_record(0x0031, &0_u32.to_le_bytes()));
    out.extend(vba_dir_record(0x001E, &0_u32.to_le_bytes()));
    out.extend(vba_dir_record(0x002C, &0xFFFF_u16.to_le_bytes()));
    if module.kind == "class" {
        out.extend(vba_dir_record(0x0022, &[]));
    } else {
        out.extend(vba_dir_record(0x0021, &[]));
    }
    out.extend(vba_dir_record(0x002B, &[]));
    out
}

fn vba_dir_record(id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + payload.len());
    out.extend(id.to_le_bytes());
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    out
}

fn dir_module_matches_source_module(candidate: &DirModule, module: &SourceModule) -> bool {
    if !module.stream_name.is_empty()
        && candidate
            .stream_name
            .eq_ignore_ascii_case(&module.stream_name)
    {
        return true;
    }
    !module.name.is_empty() && candidate.name.eq_ignore_ascii_case(&module.name)
}

pub(super) fn parse_dir_stream(data: &[u8]) -> Result<(i32, Vec<DirModule>, Vec<String>), String> {
    let modules_record = find_project_modules_record(data)?;
    let (code_page, found_code_page) = find_project_code_page(data, modules_record.record_start);
    let mut reader = DirReader {
        data,
        pos: modules_record.modules_start,
        code_page,
        modules: Vec::new(),
        warnings: Vec::new(),
    };
    reader.parse_modules(modules_record.count)?;
    if !found_code_page {
        reader.warnings.push(
            "PROJECTCODEPAGE record was not found before PROJECTMODULES; defaulted to Windows-1252"
                .to_string(),
        );
    }
    Ok((reader.code_page, reader.modules, reader.warnings))
}

impl<'a> DirReader<'a> {
    fn parse_modules(&mut self, count: usize) -> Result<(), String> {
        for _ in 0..count {
            let module = self.parse_module()?;
            self.modules.push(module);
        }
        Ok(())
    }

    fn parse_module(&mut self) -> Result<DirModule, String> {
        let mut module = DirModule::default();
        while self.remaining() >= 2 {
            let id = read_u16(self.data, self.pos)?;
            if id == 0x002B {
                if self.remaining() < 6 {
                    return Err("module terminator is truncated".to_string());
                }
                self.pos += 6;
                if module.stream_name.is_empty() {
                    module.stream_name.clone_from(&module.name);
                    self.warnings.push(format!(
                        "module {:?} did not include MODULESTREAMNAME",
                        module.name
                    ));
                }
                return Ok(module);
            }
            if self.remaining() < 6 {
                return Err(format!("module record 0x{id:04x} is truncated"));
            }
            let size = read_u32(self.data, self.pos + 2)? as usize;
            let payload_start = self.pos + 6;
            let payload_end = payload_start + size;
            if payload_end > self.data.len() {
                return Err(format!("module record 0x{id:04x} exceeds dir stream size"));
            }
            let payload = &self.data[payload_start..payload_end];
            match id {
                0x0019 => module.name = decode_mbcs(payload, self.code_page),
                0x0047 => {
                    let name = decode_utf16_le(payload);
                    if !name.is_empty() {
                        module.name = name;
                    }
                }
                0x001A => module.stream_name = decode_mbcs(payload, self.code_page),
                0x0032 => {
                    let name = decode_utf16_le(payload);
                    if !name.is_empty() {
                        module.stream_name = name;
                    }
                }
                0x0031 => {
                    if payload.len() < 4 {
                        return Err("MODULEOFFSET record is too short".to_string());
                    }
                    module.source_offset = read_u32(payload, 0)?;
                }
                0x0021 => module.kind = "standard".to_string(),
                0x0022 => module.kind = "class".to_string(),
                _ => {}
            }
            self.pos = payload_end;
        }
        Err("module record terminated unexpectedly".to_string())
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
}

fn find_project_modules_record(data: &[u8]) -> Result<ProjectModulesRecord, String> {
    for pos in 0..data.len().saturating_sub(7) {
        if read_u16(data, pos)? != 0x000F {
            continue;
        }
        if read_u32(data, pos + 2)? != 2 {
            continue;
        }
        let count = usize::from(read_u16(data, pos + 6)?);
        if count == 0 {
            continue;
        }
        let Ok(modules_start) = skip_project_cookie(data, pos + 8) else {
            continue;
        };
        let mut modules_end = modules_start;
        let mut ok = true;
        for _ in 0..count {
            match read_dir_module_block(data, modules_end) {
                Ok((_, block_end)) => modules_end = block_end,
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return Ok(ProjectModulesRecord {
                record_start: pos,
                count_payload: pos + 6,
                count,
                modules_start,
                modules_end,
            });
        }
    }
    Err("PROJECTMODULES record not found in VBA dir stream".to_string())
}

fn read_dir_module_block(data: &[u8], mut pos: usize) -> Result<(DirModule, usize), String> {
    let mut module = DirModule::default();
    while data.len().saturating_sub(pos) >= 2 {
        let id = read_u16(data, pos)?;
        if id == 0x002B {
            if data.len().saturating_sub(pos) < 6 {
                return Err("module terminator is truncated".to_string());
            }
            if module.stream_name.is_empty() {
                module.stream_name.clone_from(&module.name);
            }
            return Ok((module, pos + 6));
        }
        if data.len().saturating_sub(pos) < 6 {
            return Err(format!("module record 0x{id:04x} is truncated"));
        }
        let size = read_u32(data, pos + 2)? as usize;
        let payload_start = pos + 6;
        let payload_end = payload_start + size;
        if payload_end > data.len() {
            return Err(format!("module record 0x{id:04x} exceeds dir stream size"));
        }
        let payload = &data[payload_start..payload_end];
        match id {
            0x0019 => module.name = decode_mbcs(payload, 1252),
            0x0047 => {
                let name = decode_utf16_le(payload);
                if !name.is_empty() {
                    module.name = name;
                }
            }
            0x001A => module.stream_name = decode_mbcs(payload, 1252),
            0x0032 => {
                let name = decode_utf16_le(payload);
                if !name.is_empty() {
                    module.stream_name = name;
                }
            }
            0x0031 => {
                if payload.len() < 4 {
                    return Err("MODULEOFFSET record is too short".to_string());
                }
                module.source_offset = read_u32(payload, 0)?;
            }
            0x0021 => module.kind = "standard".to_string(),
            0x0022 => module.kind = "class".to_string(),
            _ => {}
        }
        pos = payload_end;
    }
    Err("module record terminated unexpectedly".to_string())
}

fn find_project_code_page(data: &[u8], end: usize) -> (i32, bool) {
    let end = end.min(data.len());
    for pos in 0..end.saturating_sub(7) {
        if read_u16(data, pos).unwrap_or_default() != 0x0003 {
            continue;
        }
        if read_u32(data, pos + 2).unwrap_or_default() != 2 {
            continue;
        }
        let code_page = i32::from(read_u16(data, pos + 6).unwrap_or_default());
        if code_page > 0 {
            return (code_page, true);
        }
    }
    (1252, false)
}

fn skip_project_cookie(data: &[u8], pos: usize) -> Result<usize, String> {
    if data.len().saturating_sub(pos) < 6 || read_u16(data, pos)? != 0x0013 {
        return Ok(pos);
    }
    let size = read_u32(data, pos + 2)? as usize;
    let record_end = pos + 6 + size;
    if record_end > data.len() {
        return Err("PROJECTCOOKIE record exceeds dir stream size".to_string());
    }
    Ok(record_end)
}
