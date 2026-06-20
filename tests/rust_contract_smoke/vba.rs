// VBA package-level parity tests live in a child module so the opaque macro
// wiring surface can grow without bloating the parent harness.
use super::*;
use std::collections::BTreeMap;

#[test]
fn vba_source_readback_inspect_list_extract_matches_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-vba-source-{}-{suffix}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let bin_path = temp_dir.join("vbaProject.bin");
    fs::write(&bin_path, synthetic_vba_project_bin()).expect("write synthetic vbaProject.bin");
    let bin = bin_path.to_string_lossy().to_string();

    let (go_code, go_stdout, go_stderr) =
        run_go_ooxml(&["--json", "vba", "inspect-bin", &bin, "--family", "xlsx"]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "vba", "inspect-bin", &bin, "--family", "xlsx"]);
    assert_eq!(rust_code, go_code, "inspect-bin exit");
    assert_eq!(rust_stderr, go_stderr, "inspect-bin stderr");
    assert_eq!(rust_stdout, go_stdout, "inspect-bin stdout");

    let go_in_path = temp_dir.join("go-input.xlsx");
    let rust_in_path = temp_dir.join("rust-input.xlsx");
    let go_xlsm_path = temp_dir.join("go-output.xlsm");
    let rust_xlsm_path = temp_dir.join("rust-output.xlsm");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");

    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_xlsm = go_xlsm_path.to_string_lossy().to_string();
    let rust_xlsm = rust_xlsm_path.to_string_lossy().to_string();

    let (go_code, _, go_stderr) = run_go_ooxml(&[
        "--json", "vba", "attach", &go_in, "--bin", &bin, "--out", &go_xlsm,
    ]);
    let (rust_code, _, rust_stderr) = run_ooxml(&[
        "--json", "vba", "attach", &rust_in, "--bin", &bin, "--out", &rust_xlsm,
    ]);
    assert_eq!(rust_code, go_code, "attach parseable synthetic bin exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "attach parseable synthetic bin stderr"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "list", &go_xlsm]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "vba", "list", &rust_xlsm]);
    assert_eq!(rust_code, go_code, "list exit");
    assert_eq!(rust_stderr, go_stderr, "list stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust list stdout"), &rust_xlsm, "[XLSM]"),
        scrub_path(go_stdout.expect("go list stdout"), &go_xlsm, "[XLSM]"),
        "list stdout"
    );

    let go_extract_dir = temp_dir.join("go-modules");
    let rust_extract_dir = temp_dir.join("rust-modules");
    let go_extract = go_extract_dir.to_string_lossy().to_string();
    let rust_extract = rust_extract_dir.to_string_lossy().to_string();
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "extract",
        &go_xlsm,
        "--out-dir",
        &go_extract,
        "--module",
        "module:Module1",
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "extract",
        &rust_xlsm,
        "--out-dir",
        &rust_extract,
        "--module",
        "module:Module1",
    ]);
    assert_eq!(rust_code, go_code, "extract exit");
    assert_eq!(rust_stderr, go_stderr, "extract stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust extract stdout"),
            &[(&rust_xlsm, "[XLSM]"), (&rust_extract, "[DIR]")]
        ),
        scrub_paths(
            go_stdout.expect("go extract stdout"),
            &[(&go_xlsm, "[XLSM]"), (&go_extract, "[DIR]")]
        ),
        "extract stdout"
    );
    assert_eq!(
        fs::read_to_string(rust_extract_dir.join("Module1.bas")).expect("rust Module1"),
        fs::read_to_string(go_extract_dir.join("Module1.bas")).expect("go Module1"),
        "extracted Module1 source"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "list", &go_in]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "vba", "list", &rust_in]);
    assert_eq!(rust_code, go_code, "missing macro list exit");
    assert_eq!(rust_stdout, go_stdout, "missing macro list stdout");
    assert_eq!(
        scrub_path(
            rust_stderr.expect("rust missing macro stderr"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(go_stderr.expect("go missing macro stderr"), &go_in, "[IN]"),
        "missing macro list stderr"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn vba_opaque_attach_extract_remove_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-vba-opaque-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let bin_path = temp_dir.join("vbaProject.bin");
    let payload = b"not-real-vba-but-nonempty";
    fs::write(&bin_path, payload).expect("write vbaProject.bin");
    let go_in_path = temp_dir.join("go-input.xlsx");
    let rust_in_path = temp_dir.join("rust-input.xlsx");
    let go_xlsm_path = temp_dir.join("go-output.xlsm");
    let rust_xlsm_path = temp_dir.join("rust-output.xlsm");
    let go_extract_path = temp_dir.join("go-extract.bin");
    let rust_extract_path = temp_dir.join("rust-extract.bin");
    let go_removed_path = temp_dir.join("go-removed.xlsx");
    let rust_removed_path = temp_dir.join("rust-removed.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");

    let bin = bin_path.to_string_lossy().to_string();
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_xlsm = go_xlsm_path.to_string_lossy().to_string();
    let rust_xlsm = rust_xlsm_path.to_string_lossy().to_string();
    let go_extract = go_extract_path.to_string_lossy().to_string();
    let rust_extract = rust_extract_path.to_string_lossy().to_string();
    let go_removed = go_removed_path.to_string_lossy().to_string();
    let rust_removed = rust_removed_path.to_string_lossy().to_string();

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "inspect", &go_in]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "vba", "inspect", &rust_in]);
    assert_eq!(rust_code, go_code, "initial inspect exit");
    assert_eq!(rust_stderr, go_stderr, "initial inspect stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust initial inspect"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go initial inspect"), &go_in, "[IN]"),
        "initial inspect stdout"
    );

    let go_attach_args = [
        "--json", "vba", "attach", &go_in, "--bin", &bin, "--out", &go_xlsm,
    ];
    let rust_attach_args = [
        "--json", "vba", "attach", &rust_in, "--bin", &bin, "--out", &rust_xlsm,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_attach_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_attach_args);
    assert_eq!(rust_code, go_code, "attach exit");
    assert_eq!(rust_stderr, go_stderr, "attach stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust attach stdout"),
            &[(&rust_in, "[IN]"), (&rust_xlsm, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go attach stdout"),
            &[(&go_in, "[IN]"), (&go_xlsm, "[OUT]")]
        ),
        "attach stdout"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "inspect", &go_xlsm]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "vba", "inspect", &rust_xlsm]);
    assert_eq!(rust_code, go_code, "attached inspect exit");
    assert_eq!(rust_stderr, go_stderr, "attached inspect stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust attached inspect"),
            &rust_xlsm,
            "[OUT]"
        ),
        scrub_path(go_stdout.expect("go attached inspect"), &go_xlsm, "[OUT]"),
        "attached inspect stdout"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "extract-bin",
        &go_xlsm,
        "--out",
        &go_extract,
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "extract-bin",
        &rust_xlsm,
        "--out",
        &rust_extract,
    ]);
    assert_eq!(rust_code, go_code, "extract-bin exit");
    assert_eq!(rust_stderr, go_stderr, "extract-bin stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust extract-bin stdout"),
            &[(&rust_xlsm, "[OUT]"), (&rust_extract, "[BIN]")]
        ),
        scrub_paths(
            go_stdout.expect("go extract-bin stdout"),
            &[(&go_xlsm, "[OUT]"), (&go_extract, "[BIN]")]
        ),
        "extract-bin stdout"
    );
    assert_eq!(
        fs::read(&rust_extract_path).expect("rust extracted bytes"),
        fs::read(&go_extract_path).expect("go extracted bytes"),
        "extracted vbaProject.bin bytes"
    );

    let (go_code, go_stdout, go_stderr) =
        run_go_ooxml(&["--json", "vba", "remove", &go_xlsm, "--out", &go_removed]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "remove",
        &rust_xlsm,
        "--out",
        &rust_removed,
    ]);
    assert_eq!(rust_code, go_code, "remove exit");
    assert_eq!(rust_stderr, go_stderr, "remove stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust remove stdout"),
            &[(&rust_xlsm, "[XLSM]"), (&rust_removed, "[REMOVED]")]
        ),
        scrub_paths(
            go_stdout.expect("go remove stdout"),
            &[(&go_xlsm, "[XLSM]"), (&go_removed, "[REMOVED]")]
        ),
        "remove stdout"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "inspect", &go_removed]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "vba", "inspect", &rust_removed]);
    assert_eq!(rust_code, go_code, "removed inspect exit");
    assert_eq!(rust_stderr, go_stderr, "removed inspect stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust removed inspect"),
            &rust_removed,
            "[REMOVED]"
        ),
        scrub_path(
            go_stdout.expect("go removed inspect"),
            &go_removed,
            "[REMOVED]"
        ),
        "removed inspect stdout"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "attach",
        &go_in,
        "--bin",
        &bin,
        "--dry-run",
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "attach",
        &rust_in,
        "--bin",
        &bin,
        "--dry-run",
    ]);
    assert_eq!(rust_code, go_code, "attach dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "attach dry-run stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust attach dry-run"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go attach dry-run"), &go_in, "[IN]"),
        "attach dry-run stdout"
    );

    let (go_code, go_stdout, go_stderr) =
        run_go_ooxml(&["--json", "vba", "remove", &go_xlsm, "--dry-run"]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "vba", "remove", &rust_xlsm, "--dry-run"]);
    assert_eq!(rust_code, go_code, "remove dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "remove dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust remove dry-run"),
            &rust_xlsm,
            "[XLSM]"
        ),
        scrub_path(go_stdout.expect("go remove dry-run"), &go_xlsm, "[XLSM]"),
        "remove dry-run stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[derive(Clone)]
struct SyntheticVbaModule {
    name: &'static str,
    stream_name: &'static str,
    kind: &'static str,
    source: &'static str,
}

#[derive(Clone)]
struct SyntheticCfbEntry {
    name: String,
    object_type: u8,
    left: u32,
    right: u32,
    child: u32,
    start_sector: u32,
    size: u64,
}

fn synthetic_vba_project_bin() -> Vec<u8> {
    let modules = vec![
        SyntheticVbaModule {
            name: "Module1",
            stream_name: "Module1",
            kind: "standard",
            source: "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
        },
        SyntheticVbaModule {
            name: "Class1",
            stream_name: "Class1",
            kind: "class",
            source: "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
        },
    ];
    let mut streams = BTreeMap::new();
    streams.insert(
        "VBA/dir".to_string(),
        compressed_vba_literals(&synthetic_dir_stream(&modules)),
    );
    streams.insert("VBA/_VBA_PROJECT".to_string(), vec![0xCC, 0x61]);
    for module in modules {
        streams.insert(
            format!("VBA/{}", module.stream_name),
            compressed_vba_literals(module.source.as_bytes()),
        );
    }
    synthetic_cfb(streams)
}

fn synthetic_dir_stream(modules: &[SyntheticVbaModule]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(vba_dir_record(0x0003, &le16(1252)));
    out.extend(vba_dir_record(0x000F, &le16(modules.len() as u16)));
    for module in modules {
        out.extend(vba_dir_record(0x0019, module.name.as_bytes()));
        out.extend(vba_dir_record(0x0047, &utf16le_bytes(module.name)));
        out.extend(vba_dir_record(0x001A, module.stream_name.as_bytes()));
        out.extend(vba_dir_record(0x0032, &utf16le_bytes(module.stream_name)));
        out.extend(vba_dir_record(0x0031, &le32(0)));
        if module.kind == "class" {
            out.extend(vba_dir_record(0x0022, &[]));
        } else {
            out.extend(vba_dir_record(0x0021, &[]));
        }
        out.extend(vba_dir_record(0x002B, &[]));
    }
    out.extend(vba_dir_record(0x0010, &[]));
    out
}

fn vba_dir_record(id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + payload.len());
    out.extend(id.to_le_bytes());
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    out
}

fn compressed_vba_literals(mut raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x01];
    while !raw.is_empty() {
        let literal_len = raw.len().min(3600);
        let literal_chunk = &raw[..literal_len];
        let mut chunk = Vec::new();
        let mut offset = 0;
        while offset < literal_chunk.len() {
            let n = (literal_chunk.len() - offset).min(8);
            chunk.push(0x00);
            chunk.extend(&literal_chunk[offset..offset + n]);
            offset += n;
        }
        let header = (chunk.len() as u16 - 1) | 0x3000 | 0x8000;
        out.extend(header.to_le_bytes());
        out.extend(chunk);
        raw = &raw[literal_len..];
    }
    out
}

fn synthetic_cfb(streams: BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    const SECTOR_SIZE: usize = 512;
    const NO_STREAM: u32 = 0xFFFF_FFFF;
    const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
    const FAT_SECTOR: u32 = 0xFFFF_FFFD;

    let mut names = vec!["dir".to_string(), "_VBA_PROJECT".to_string()];
    let mut module_names = streams
        .keys()
        .filter_map(|path| path.strip_prefix("VBA/"))
        .filter(|name| *name != "dir" && *name != "_VBA_PROJECT")
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    module_names.sort();
    names.extend(module_names);

    let mut sectors = vec![vec![0; SECTOR_SIZE]];
    let mut entries = vec![
        SyntheticCfbEntry {
            name: "Root Entry".to_string(),
            object_type: 5,
            left: NO_STREAM,
            right: NO_STREAM,
            child: 1,
            start_sector: END_OF_CHAIN,
            size: 0,
        },
        SyntheticCfbEntry {
            name: "VBA".to_string(),
            object_type: 1,
            left: NO_STREAM,
            right: NO_STREAM,
            child: 2,
            start_sector: END_OF_CHAIN,
            size: 0,
        },
    ];
    for (idx, name) in names.iter().enumerate() {
        let data = streams
            .get(&format!("VBA/{name}"))
            .unwrap_or_else(|| panic!("missing synthetic stream {name}"));
        let start = sectors.len() as u32;
        let mut padded = data.clone();
        while !padded.len().is_multiple_of(SECTOR_SIZE) {
            padded.push(0);
        }
        for chunk in padded.chunks(SECTOR_SIZE) {
            sectors.push(chunk.to_vec());
        }
        let right = if idx < names.len() - 1 {
            entries.len() as u32 + 1
        } else {
            NO_STREAM
        };
        entries.push(SyntheticCfbEntry {
            name: name.clone(),
            object_type: 2,
            left: NO_STREAM,
            right,
            child: NO_STREAM,
            start_sector: start,
            size: data.len() as u64,
        });
    }

    let dir_start = sectors.len() as u32;
    let mut dir_data = Vec::new();
    for entry in &entries {
        dir_data.extend(cfb_directory_entry(entry));
    }
    while !dir_data.len().is_multiple_of(SECTOR_SIZE) {
        dir_data.push(0);
    }
    for chunk in dir_data.chunks(SECTOR_SIZE) {
        sectors.push(chunk.to_vec());
    }

    let mut fat = vec![END_OF_CHAIN; sectors.len()];
    fat[0] = FAT_SECTOR;
    for entry in &entries {
        if entry.object_type != 2 || entry.size == 0 {
            continue;
        }
        let count = (entry.size as usize).div_ceil(SECTOR_SIZE);
        for i in 0..count.saturating_sub(1) {
            fat[entry.start_sector as usize + i] = entry.start_sector + i as u32 + 1;
        }
    }
    for i in 0..(sectors.len() - dir_start as usize).saturating_sub(1) {
        fat[dir_start as usize + i] = dir_start + i as u32 + 1;
    }
    for (idx, value) in fat.iter().enumerate() {
        sectors[0][idx * 4..idx * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    for idx in fat.len()..SECTOR_SIZE / 4 {
        sectors[0][idx * 4..idx * 4 + 4].copy_from_slice(&NO_STREAM.to_le_bytes());
    }

    let mut header = vec![0; 512];
    header[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
    header[24..26].copy_from_slice(&0x003E_u16.to_le_bytes());
    header[26..28].copy_from_slice(&0x0003_u16.to_le_bytes());
    header[28..30].copy_from_slice(&0xFFFE_u16.to_le_bytes());
    header[30..32].copy_from_slice(&9_u16.to_le_bytes());
    header[32..34].copy_from_slice(&6_u16.to_le_bytes());
    header[44..48].copy_from_slice(&1_u32.to_le_bytes());
    header[48..52].copy_from_slice(&dir_start.to_le_bytes());
    header[56..60].copy_from_slice(&0_u32.to_le_bytes());
    header[60..64].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
    header[68..72].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
    header[76..80].copy_from_slice(&0_u32.to_le_bytes());
    for offset in (80..512).step_by(4) {
        header[offset..offset + 4].copy_from_slice(&NO_STREAM.to_le_bytes());
    }

    let mut out = header;
    for sector in sectors {
        out.extend(sector);
    }
    out
}

fn cfb_directory_entry(entry: &SyntheticCfbEntry) -> Vec<u8> {
    let mut out = vec![0; 128];
    let name_bytes = utf16le_bytes(&(entry.name.clone() + "\0"));
    out[..name_bytes.len()].copy_from_slice(&name_bytes);
    out[64..66].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    out[66] = entry.object_type;
    out[67] = 1;
    out[68..72].copy_from_slice(&entry.left.to_le_bytes());
    out[72..76].copy_from_slice(&entry.right.to_le_bytes());
    out[76..80].copy_from_slice(&entry.child.to_le_bytes());
    out[116..120].copy_from_slice(&entry.start_sector.to_le_bytes());
    out[120..124].copy_from_slice(&(entry.size as u32).to_le_bytes());
    out
}

fn utf16le_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

fn le16(value: u16) -> [u8; 2] {
    value.to_le_bytes()
}

fn le32(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}
