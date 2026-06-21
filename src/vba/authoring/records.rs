use super::codec::utf16le_bytes;
use super::model::{VbaModuleKind, VbaModuleModel, VbaProjectModel};

const PROJECT_GUID: &str = "{917DED54-440B-4FD1-A5C1-74ACF261E600}";
const PROJECT_VERSION_MAJOR: u32 = 0x6C59D84B;
const PROJECT_VERSION_MINOR: u16 = 0x0004;
const WRITE_COOKIE: u16 = 0xFFFF;

pub(super) fn render_project_stream(project: &VbaProjectModel) -> Vec<u8> {
    let mut lines = Vec::new();
    lines.push(format!("ID=\"{PROJECT_GUID}\""));
    for module in &project.modules {
        lines.push(format!(
            "{}={}",
            module.kind.project_key(),
            project_stream_module_value(module)
        ));
    }
    lines.push(format!("Name=\"{}\"", project.project_name));
    lines.push("HelpContextID=\"0\"".to_string());
    lines.push("VersionCompatible32=\"393222000\"".to_string());
    lines.push("CMG=\"0705D8E3D8EDDBF1DBF1DBF1DBF1\"".to_string());
    lines.push("DPB=\"0E0CD1ECDFF4E7F5E7F5E7\"".to_string());
    lines.push("GC=\"1517CAF1D6F9D7F9D706\"".to_string());
    lines.push(String::new());
    lines.push("[Host Extender Info]".to_string());
    lines.push("&H00000001={3832D640-CF90-11CF-8E43-00A0C911005A};VBE;&H00000000".to_string());
    lines.push(String::new());
    lines.push("[Workspace]".to_string());
    for module in &project.modules {
        lines.push(format!("{}=0, 0, 0, 0, C", module.name));
    }
    let mut text = lines.join("\r\n");
    text.push_str("\r\n");
    text.into_bytes()
}

fn project_stream_module_value(module: &VbaModuleModel) -> String {
    if module.kind == VbaModuleKind::Document {
        format!("{}/&H00000000", module.name)
    } else {
        module.name.clone()
    }
}

pub(super) fn render_project_wm_stream(project: &VbaProjectModel) -> Vec<u8> {
    let mut out = Vec::new();
    for module in &project.modules {
        out.extend_from_slice(module.name.as_bytes());
        out.push(0);
        out.extend(utf16le_bytes(&module.name));
        out.extend([0, 0]);
    }
    out.extend([0, 0]);
    out
}

pub(super) fn render_vba_project_stream() -> Vec<u8> {
    vec![0xCC, 0x61, 0xFF, 0xFF, 0x00, 0x01, 0x00]
}

pub(super) fn render_dir_stream(project: &VbaProjectModel) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(fixed_u32_record(0x0001, 0x00000003));
    out.extend(fixed_u32_record(0x004A, 0x00000006));
    out.extend(fixed_u32_record(0x0002, 0x00000409));
    out.extend(fixed_u32_record(0x0014, 0x00000409));
    out.extend(fixed_u16_record(0x0003, project.code_page));
    out.extend(variable_bytes_record(
        0x0004,
        project.project_name.as_bytes(),
    ));
    out.extend(dual_string_record(0x0005, 0x0040, b""));
    out.extend(dual_string_record(0x0006, 0x003D, b""));
    out.extend(fixed_u32_record(0x0007, 0));
    out.extend(fixed_u32_record(0x0008, 0));
    out.extend(project_version_record());
    out.extend(dual_string_record(0x000C, 0x003C, b""));
    out.extend(render_registered_references());
    out.extend(fixed_u16_record(0x000F, project.modules.len() as u16));
    out.extend(fixed_u16_record(0x0013, WRITE_COOKIE));
    for module in &project.modules {
        out.extend(vba_dir_record(0x0019, module.name.as_bytes()));
        out.extend(vba_dir_record(0x0047, &utf16le_bytes(&module.name)));
        out.extend(vba_dir_record(0x001A, module.stream_name.as_bytes()));
        out.extend(vba_dir_record(0x0032, &utf16le_bytes(&module.stream_name)));
        out.extend(vba_dir_record(0x001C, &[]));
        out.extend(vba_dir_record(0x0048, &[]));
        out.extend(vba_dir_record(0x0031, &0_u32.to_le_bytes()));
        out.extend(vba_dir_record(0x001E, &0_u32.to_le_bytes()));
        out.extend(vba_dir_record(0x002C, &WRITE_COOKIE.to_le_bytes()));
        out.extend(vba_dir_record(module.kind.dir_record_id(), &[]));
        if module.kind == VbaModuleKind::Class {
            out.extend(vba_dir_record(0x0028, &[]));
        }
        out.extend(vba_dir_record(0x002B, &[]));
    }
    out.extend(vba_dir_record(0x0010, &[]));
    out
}

fn render_registered_references() -> Vec<u8> {
    [
        (
            "stdole",
            r#"*\G{00020430-0000-0000-C000-000000000046}#2.0#0#C:\Windows\System32\stdole2.tlb#OLE Automation"#,
        ),
        (
            "Office",
            r#"*\G{2DF8D04C-5BFA-101B-BDE5-00AA0044DE52}#2.0#0#C:\Program Files\Common Files\Microsoft Shared\OFFICE16\MSO.DLL#Microsoft Office 16.0 Object Library"#,
        ),
    ]
    .into_iter()
    .flat_map(|(name, libid)| registered_reference_record(name, libid))
    .collect()
}

fn registered_reference_record(name: &str, libid: &str) -> Vec<u8> {
    let mut out = dual_string_record(0x0016, 0x003E, name.as_bytes());
    let libid = libid.as_bytes();
    let mut payload = Vec::with_capacity(4 + libid.len() + 6);
    payload.extend((libid.len() as u32).to_le_bytes());
    payload.extend(libid);
    payload.extend(0_u32.to_le_bytes());
    payload.extend(0_u16.to_le_bytes());
    out.extend(vba_dir_record(0x000D, &payload));
    out
}

fn fixed_u16_record(id: u16, value: u16) -> Vec<u8> {
    vba_dir_record(id, &value.to_le_bytes())
}

fn fixed_u32_record(id: u16, value: u32) -> Vec<u8> {
    vba_dir_record(id, &value.to_le_bytes())
}

fn variable_bytes_record(id: u16, bytes: &[u8]) -> Vec<u8> {
    vba_dir_record(id, bytes)
}

fn dual_string_record(id: u16, reserved: u16, mbcs: &[u8]) -> Vec<u8> {
    let unicode = String::from_utf8_lossy(mbcs);
    let unicode = utf16le_bytes(&unicode);
    let mut out = Vec::with_capacity(12 + mbcs.len() + unicode.len());
    out.extend(id.to_le_bytes());
    out.extend((mbcs.len() as u32).to_le_bytes());
    out.extend(mbcs);
    out.extend(reserved.to_le_bytes());
    out.extend((unicode.len() as u32).to_le_bytes());
    out.extend(unicode);
    out
}

fn project_version_record() -> Vec<u8> {
    let mut out = Vec::with_capacity(12);
    out.extend(0x0009_u16.to_le_bytes());
    out.extend(0x00000004_u32.to_le_bytes());
    out.extend(PROJECT_VERSION_MAJOR.to_le_bytes());
    out.extend(PROJECT_VERSION_MINOR.to_le_bytes());
    out
}

fn vba_dir_record(id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + payload.len());
    out.extend(id.to_le_bytes());
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vba::authoring::model::{VbaModuleKind, VbaModuleModel, VbaProjectModel};

    fn two_module_project() -> VbaProjectModel {
        VbaProjectModel::xlsx(vec![
            VbaModuleModel::excel_workbook_document(),
            VbaModuleModel::excel_sheet_document("Sheet1"),
            VbaModuleModel::standard(
                "Module1",
                b"Attribute VB_Name = \"Module1\"\r\nSub Hello()\r\nEnd Sub\r\n".to_vec(),
            ),
            VbaModuleModel::class(
                "Worker",
                b"Attribute VB_Name = \"Worker\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n".to_vec(),
            ),
        ])
    }

    fn pptx_project() -> VbaProjectModel {
        VbaProjectModel::pptx(vec![VbaModuleModel::standard(
            "Module1",
            b"Attribute VB_Name = \"Module1\"\r\nSub Hello()\r\nEnd Sub\r\n".to_vec(),
        )])
    }

    fn docx_project() -> VbaProjectModel {
        VbaProjectModel::docx(vec![VbaModuleModel::standard(
            "Module1",
            b"Attribute VB_Name = \"Module1\"\r\nSub Hello()\r\nEnd Sub\r\n".to_vec(),
        )])
    }

    #[test]
    fn project_stream_declares_modules_and_workspace() {
        let text = String::from_utf8(render_project_stream(&two_module_project())).unwrap();
        assert!(text.contains("Document=ThisWorkbook/&H00000000\r\n"));
        assert!(text.contains("Document=Sheet1/&H00000000\r\n"));
        assert!(text.contains("Module=Module1\r\n"));
        assert!(text.contains("Class=Worker\r\n"));
        assert!(text.contains("Name=\"VBAProject\"\r\n"));
        assert!(text.contains("[Workspace]\r\n"));
        assert!(text.contains("Module1=0, 0, 0, 0, C\r\n"));
        assert!(text.contains("Worker=0, 0, 0, 0, C\r\n"));
    }

    #[test]
    fn pptx_project_stream_declares_only_user_modules() {
        let text = String::from_utf8(render_project_stream(&pptx_project())).unwrap();
        assert!(text.contains("Module=Module1\r\n"));
        assert!(!text.contains("Document=ThisWorkbook"));
        assert!(!text.contains("Document=Sheet1"));
        assert!(text.contains("Module1=0, 0, 0, 0, C\r\n"));
    }

    #[test]
    fn docx_project_stream_declares_only_standard_modules() {
        let text = String::from_utf8(render_project_stream(&docx_project())).unwrap();
        assert!(text.contains("Module=Module1\r\n"));
        assert!(!text.contains("Document=ThisDocument"));
        assert!(!text.contains("Document=ThisWorkbook"));
        assert!(text.contains("Module1=0, 0, 0, 0, C\r\n"));
    }

    #[test]
    fn project_wm_stream_contains_ascii_name_utf16_display_pairs() {
        let stream = render_project_wm_stream(&two_module_project());
        let module1 = project_wm_pair("Module1");
        let worker = project_wm_pair("Worker");
        assert!(
            stream
                .windows(module1.len())
                .any(|window| window == module1)
        );
        assert!(stream.windows(worker.len()).any(|window| window == worker));
        assert!(stream.ends_with(&[0, 0]));
    }

    #[test]
    fn vba_project_stream_uses_spec_write_version_without_cache() {
        assert_eq!(
            render_vba_project_stream(),
            vec![0xCC, 0x61, 0xFF, 0xFF, 0x00, 0x01, 0x00]
        );
    }

    #[test]
    fn primitive_record_writers_emit_exact_little_endian_bytes() {
        assert_eq!(
            fixed_u16_record(0x0003, 1252),
            vec![0x03, 0x00, 0x02, 0x00, 0x00, 0x00, 0xE4, 0x04,]
        );
        assert_eq!(
            fixed_u32_record(0x004A, 6),
            vec![0x4A, 0x00, 0x04, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00,]
        );
        assert_eq!(
            variable_bytes_record(0x0004, b"VBA"),
            vec![0x04, 0x00, 0x03, 0x00, 0x00, 0x00, b'V', b'B', b'A']
        );
        assert_eq!(
            vba_dir_record(0x002B, &[]),
            vec![0x2B, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn complex_record_writers_emit_exact_binary_shapes() {
        assert_eq!(
            dual_string_record(0x0016, 0x003E, b"Ref"),
            vec![
                0x16, 0x00, 0x03, 0x00, 0x00, 0x00, b'R', b'e', b'f', 0x3E, 0x00, 0x06, 0x00, 0x00,
                0x00, b'R', 0x00, b'e', 0x00, b'f', 0x00,
            ]
        );
        assert_eq!(
            project_version_record(),
            vec![
                0x09, 0x00, 0x04, 0x00, 0x00, 0x00, 0x4B, 0xD8, 0x59, 0x6C, 0x04, 0x00,
            ]
        );

        let reference = registered_reference_record("stdole", r#"*\G{00020430}#2.0"#);
        let name_part_len = dual_string_record(0x0016, 0x003E, b"stdole").len();
        assert_eq!(&reference[..2], &0x0016_u16.to_le_bytes());
        assert_eq!(read_u16_at(&reference, name_part_len), 0x000D);
        assert_eq!(
            read_u32_at(&reference, name_part_len + 6),
            r#"*\G{00020430}#2.0"#.len() as u32
        );
        assert_eq!(
            &reference[name_part_len + 10..name_part_len + 10 + r#"*\G{00020430}#2.0"#.len()],
            r#"*\G{00020430}#2.0"#.as_bytes()
        );
    }

    #[test]
    fn dir_stream_uses_existing_parser_record_ids_and_moduleoffset_zero() {
        let dir = render_dir_stream(&two_module_project());
        assert!(contains_record(&dir, 0x0001, &3_u32.to_le_bytes()));
        assert!(contains_record(&dir, 0x004A, &6_u32.to_le_bytes()));
        assert!(contains_record(&dir, 0x0003, &1252_u16.to_le_bytes()));
        assert!(contains_record(&dir, 0x000F, &4_u16.to_le_bytes()));
        assert!(contains_record(&dir, 0x0013, &WRITE_COOKIE.to_le_bytes()));
        assert!(contains_record(&dir, 0x0019, b"Module1"));
        assert!(contains_record(&dir, 0x001A, b"Worker"));
        assert!(contains_record(&dir, 0x0031, &0_u32.to_le_bytes()));
        assert_eq!(record_count(&dir, 0x002C, &WRITE_COOKIE.to_le_bytes()), 4);
        assert!(contains_record(
            &dir,
            VbaModuleKind::Standard.dir_record_id(),
            &[]
        ));
        assert!(contains_record(
            &dir,
            VbaModuleKind::Class.dir_record_id(),
            &[]
        ));
        assert_eq!(record_count(&dir, 0x0028, &[]), 1);
        assert!(contains_record(&dir, 0x0010, &[]));
    }

    #[test]
    fn dir_stream_records_are_written_in_strict_order() {
        let dir = render_dir_stream(&two_module_project());
        let mut expected = vec![
            0x0001, 0x004A, 0x0002, 0x0014, 0x0003, 0x0004, 0x0005, 0x0006, 0x0007, 0x0008, 0x0009,
            0x000C, 0x0016, 0x000D, 0x0016, 0x000D, 0x000F, 0x0013,
        ];
        expected.extend(module_record_ids(VbaModuleKind::Document));
        expected.extend(module_record_ids(VbaModuleKind::Document));
        expected.extend(module_record_ids(VbaModuleKind::Standard));
        expected.extend(module_record_ids(VbaModuleKind::Class));
        expected.push(0x0010);

        assert_eq!(record_ids_in_order(&dir), expected);
    }

    #[test]
    fn pptx_dir_stream_uses_one_standard_module() {
        let dir = render_dir_stream(&pptx_project());
        assert!(contains_record(&dir, 0x0003, &1252_u16.to_le_bytes()));
        assert!(contains_record(&dir, 0x000F, &1_u16.to_le_bytes()));
        assert!(contains_record(&dir, 0x0013, &WRITE_COOKIE.to_le_bytes()));
        assert!(contains_record(&dir, 0x0019, b"Module1"));
        assert_eq!(record_count(&dir, 0x002C, &WRITE_COOKIE.to_le_bytes()), 1);
        assert!(contains_record(
            &dir,
            VbaModuleKind::Standard.dir_record_id(),
            &[]
        ));
    }

    #[test]
    fn docx_dir_stream_uses_one_standard_module() {
        let dir = render_dir_stream(&docx_project());
        assert!(contains_record(&dir, 0x0003, &1252_u16.to_le_bytes()));
        assert!(contains_record(&dir, 0x000F, &1_u16.to_le_bytes()));
        assert!(contains_record(&dir, 0x0013, &WRITE_COOKIE.to_le_bytes()));
        assert!(contains_record(&dir, 0x0019, b"Module1"));
        assert_eq!(record_count(&dir, 0x002C, &WRITE_COOKIE.to_le_bytes()), 1);
        assert!(contains_record(
            &dir,
            VbaModuleKind::Standard.dir_record_id(),
            &[]
        ));
    }

    fn contains_record(data: &[u8], id: u16, payload: &[u8]) -> bool {
        for pos in 0..=data.len().saturating_sub(6) {
            let record_id = u16::from_le_bytes([data[pos], data[pos + 1]]);
            let size =
                u32::from_le_bytes([data[pos + 2], data[pos + 3], data[pos + 4], data[pos + 5]])
                    as usize;
            let payload_start = pos + 6;
            let payload_end = payload_start + size;
            if payload_end > data.len() {
                continue;
            }
            if record_id == id && &data[payload_start..payload_end] == payload {
                return true;
            }
        }
        false
    }

    fn record_count(data: &[u8], id: u16, payload: &[u8]) -> usize {
        (0..=data.len().saturating_sub(6))
            .filter(|pos| {
                let record_id = u16::from_le_bytes([data[*pos], data[*pos + 1]]);
                let size = u32::from_le_bytes([
                    data[*pos + 2],
                    data[*pos + 3],
                    data[*pos + 4],
                    data[*pos + 5],
                ]) as usize;
                let payload_start = *pos + 6;
                let payload_end = payload_start + size;
                payload_end <= data.len()
                    && record_id == id
                    && &data[payload_start..payload_end] == payload
            })
            .count()
    }

    fn record_ids_in_order(data: &[u8]) -> Vec<u16> {
        let mut out = Vec::new();
        let mut pos = 0;
        while pos < data.len() {
            assert!(
                pos + 6 <= data.len(),
                "record header at byte {pos} extends past dir stream"
            );
            let id = read_u16_at(data, pos);
            out.push(id);
            pos = next_record_offset(data, pos, id);
        }
        out
    }

    fn next_record_offset(data: &[u8], pos: usize, id: u16) -> usize {
        if id == 0x0009 {
            assert_eq!(read_u32_at(data, pos + 2), 4, "PROJECTVERSION size");
            assert!(
                pos + 12 <= data.len(),
                "PROJECTVERSION at byte {pos} extends past dir stream"
            );
            return pos + 12;
        }

        let size = read_u32_at(data, pos + 2) as usize;
        let payload_end = pos + 6 + size;
        assert!(
            payload_end <= data.len(),
            "record 0x{id:04x} at byte {pos} extends past dir stream"
        );
        if matches!(id, 0x0005 | 0x0006 | 0x000C | 0x0016) {
            assert!(
                payload_end + 6 <= data.len(),
                "dual-string record 0x{id:04x} at byte {pos} is missing Unicode header"
            );
            let reserved = read_u16_at(data, payload_end);
            let expected_reserved = match id {
                0x0005 => 0x0040,
                0x0006 => 0x003D,
                0x000C => 0x003C,
                0x0016 => 0x003E,
                _ => unreachable!(),
            };
            assert_eq!(
                reserved, expected_reserved,
                "dual-string reserved record for 0x{id:04x}"
            );
            let unicode_size = read_u32_at(data, payload_end + 2) as usize;
            let end = payload_end + 6 + unicode_size;
            assert!(
                end <= data.len(),
                "dual-string record 0x{id:04x} at byte {pos} extends past dir stream"
            );
            return end;
        }
        payload_end
    }

    fn read_u16_at(data: &[u8], pos: usize) -> u16 {
        u16::from_le_bytes([data[pos], data[pos + 1]])
    }

    fn read_u32_at(data: &[u8], pos: usize) -> u32 {
        u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
    }

    fn module_record_ids(kind: VbaModuleKind) -> Vec<u16> {
        let mut ids = vec![
            0x0019,
            0x0047,
            0x001A,
            0x0032,
            0x001C,
            0x0048,
            0x0031,
            0x001E,
            0x002C,
            kind.dir_record_id(),
        ];
        if kind == VbaModuleKind::Class {
            ids.push(0x0028);
        }
        ids.push(0x002B);
        ids
    }

    fn project_wm_pair(name: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(name.as_bytes());
        out.push(0);
        out.extend(utf16le_bytes(name));
        out.extend([0, 0]);
        out
    }
}
