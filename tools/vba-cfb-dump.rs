mod cfb {
    #![allow(dead_code)]
    include!("../src/vba/cfb.rs");
}

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let args = env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!("usage: vba-cfb-dump <vbaProject.bin> <out-dir>");
        std::process::exit(2);
    }

    let input = &args[1];
    let out_dir = Path::new(&args[2]);
    let data = fs::read(input).unwrap_or_else(|err| {
        eprintln!("failed to read {input}: {err}");
        std::process::exit(1);
    });
    let file = cfb::CfbFile::open(&data).unwrap_or_else(|err| {
        eprintln!("failed to parse CFB {input}: {err}");
        std::process::exit(1);
    });
    fs::create_dir_all(out_dir).unwrap_or_else(|err| {
        eprintln!("failed to create {}: {err}", out_dir.display());
        std::process::exit(1);
    });

    println!("stream\tbytes\tprefix_hex\toutput");
    for stream in file.streams() {
        let bytes = file.stream(&stream).unwrap_or_else(|err| {
            eprintln!("failed to read stream {stream}: {err}");
            std::process::exit(1);
        });
        let output = out_dir.join(format!("{}.bin", sanitize_name(&stream)));
        fs::write(&output, &bytes).unwrap_or_else(|err| {
            eprintln!("failed to write {}: {err}", output.display());
            std::process::exit(1);
        });
        println!(
            "{}\t{}\t{}\t{}",
            stream,
            bytes.len(),
            hex_prefix(&bytes, 32),
            output.display()
        );
    }
}

fn sanitize_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn hex_prefix(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}
