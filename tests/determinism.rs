use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SOURCE_DATE_EPOCH: &str = "946684800";
const SOURCE_DATE_TIMESTAMP: &str = "2000-01-01T00:00:00Z";
const DETERMINISM_CONVENTION: &str = "package outputs are byte-deterministic for identical inputs; SOURCE_DATE_EPOCH sets created and modified core-property timestamps, and timestamps are omitted when it is unset";
const TEXT_CONVENTION: &str =
    "text styling is suppressed when NO_COLOR or CI is set, TERM=dumb, or stdout is not a TTY";

fn temp_dir(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ooxml-determinism-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create determinism test directory");
    path
}

fn command(args: &[&str], environment: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ooxml"));
    command.args(args);
    for name in ["SOURCE_DATE_EPOCH", "NO_COLOR", "CI", "TERM"] {
        command.env_remove(name);
    }
    for (name, value) in environment {
        command.env(name, value);
    }
    command.output().expect("run ooxml")
}

fn run_ok(args: &[&str], environment: &[(&str, &str)]) -> Output {
    let output = command(args, environment);
    assert!(
        output.status.success(),
        "args={args:?}\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn run_json(args: &[&str], environment: &[(&str, &str)]) -> Value {
    let output = run_ok(args, environment);
    serde_json::from_slice(&output.stdout).expect("JSON stdout")
}

fn zip_text(path: &Path, part: &str) -> String {
    let mut archive =
        zip::ZipArchive::new(File::open(path).expect("open package")).expect("open ZIP package");
    let mut text = String::new();
    archive
        .by_name(part)
        .unwrap_or_else(|_| panic!("missing {part} in {}", path.display()))
        .read_to_string(&mut text)
        .expect("read package XML");
    text
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn strict_validate(path: &Path) {
    run_ok(&["validate", "--strict", path.to_str().unwrap()], &[]);
}

fn scaffold(family: &str, output: &Path, environment: &[(&str, &str)]) {
    let output = output.to_str().unwrap();
    match family {
        "pptx" => {
            run_json(
                &[
                    "--json",
                    "pptx",
                    "scaffold",
                    output,
                    "--title",
                    "Quarterly Review",
                    "--subtitle",
                    "Deterministic recipe",
                    "--theme-seed",
                    "#336699",
                ],
                environment,
            );
        }
        "xlsx" => {
            run_json(
                &[
                    "--json",
                    "xlsx",
                    "scaffold",
                    "--out",
                    output,
                    "--sheet",
                    "Sales",
                    "--sheet",
                    "Inputs",
                    "--theme-seed",
                    "#336699",
                ],
                environment,
            );
        }
        "docx" => {
            run_json(
                &[
                    "--json",
                    "docx",
                    "scaffold",
                    "--out",
                    output,
                    "--text",
                    "Quarterly review deterministic recipe",
                    "--theme-seed",
                    "#336699",
                ],
                environment,
            );
        }
        _ => unreachable!(),
    }
}

fn assert_timestamp_policy(core: &str, expected: Option<&str>) {
    match expected {
        Some(timestamp) => {
            assert!(core.contains(&format!(
                r#"<dcterms:created xsi:type="dcterms:W3CDTF">{timestamp}</dcterms:created>"#
            )));
            assert!(core.contains(&format!(
                r#"<dcterms:modified xsi:type="dcterms:W3CDTF">{timestamp}</dcterms:modified>"#
            )));
        }
        None => {
            assert!(
                !core.contains("dcterms:created"),
                "unexpected created timestamp"
            );
            assert!(
                !core.contains("dcterms:modified"),
                "unexpected modified timestamp"
            );
        }
    }
}

fn assert_golden(actual: &Value) {
    let path = Path::new("testdata/golden/determinism/family-builds.json");
    let mut bytes = serde_json::to_vec_pretty(actual).expect("serialize determinism golden");
    bytes.push(b'\n');
    if std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1") {
        fs::create_dir_all(path.parent().unwrap()).expect("create determinism golden directory");
        fs::write(path, &bytes).expect("write determinism golden");
    }
    let expected = fs::read(path).unwrap_or_else(|error| {
        panic!(
            "missing {}: {error}; rerun with UPDATE_GOLDENS=1",
            path.display()
        )
    });
    assert_eq!(bytes, expected, "determinism golden drift");
}

#[test]
fn family_recipes_are_byte_deterministic_and_source_date_epoch_is_the_only_timestamp() {
    let root = temp_dir("families");
    let mut families = Map::new();
    for family in ["pptx", "xlsx", "docx"] {
        let extension = family;
        let first = root.join(format!("{family}-first.{extension}"));
        let second = root.join(format!("{family}-second.{extension}"));
        let dated = root.join(format!("{family}-dated.{extension}"));
        scaffold(family, &first, &[]);
        scaffold(family, &second, &[]);
        scaffold(family, &dated, &[("SOURCE_DATE_EPOCH", SOURCE_DATE_EPOCH)]);
        for package in [&first, &second, &dated] {
            strict_validate(package);
        }

        let first_bytes = fs::read(&first).expect("read first package");
        let second_bytes = fs::read(&second).expect("read second package");
        assert_eq!(
            first_bytes, second_bytes,
            "{family} identical recipe inputs must produce identical bytes"
        );
        let undated_core = zip_text(&first, "docProps/core.xml");
        let dated_core = zip_text(&dated, "docProps/core.xml");
        assert_timestamp_policy(&undated_core, None);
        assert_timestamp_policy(&dated_core, Some(SOURCE_DATE_TIMESTAMP));
        assert_ne!(
            fs::read(&first).unwrap(),
            fs::read(&dated).unwrap(),
            "SOURCE_DATE_EPOCH must affect package metadata"
        );
        families.insert(
            family.to_string(),
            json!({
                "bytes": first_bytes.len(),
                "packageSha256": sha256(&first_bytes),
                "corePropertiesSha256": sha256(undated_core.as_bytes()),
                "timestampsPresentWithoutSourceDateEpoch": false,
                "sourceDateEpoch": SOURCE_DATE_EPOCH,
                "sourceDateTimestamp": SOURCE_DATE_TIMESTAMP
            }),
        );
    }
    assert_golden(&json!({
        "contractVersion": 1,
        "families": families
    }));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn automation_and_non_tty_text_outputs_are_plain_and_stable() {
    let commands: [&[&str]; 3] = [
        &["help"],
        &["doctor", "robot-docs"],
        &[
            "find",
            "Title",
            "testdata/pptx/multi-layout/presentation.pptx",
        ],
    ];
    let environments: [(&str, &[(&str, &str)]); 4] = [
        ("non-tty", &[]),
        ("NO_COLOR", &[("NO_COLOR", "1")]),
        ("CI", &[("CI", "true")]),
        ("TERM=dumb", &[("TERM", "dumb")]),
    ];
    let baseline = commands
        .iter()
        .map(|args| run_ok(args, &[]).stdout)
        .collect::<Vec<_>>();
    for (label, environment) in environments {
        for (index, args) in commands.iter().enumerate() {
            let output = run_ok(args, environment);
            assert!(!output.stdout.is_empty(), "{label} {args:?} text output");
            assert!(
                !output.stdout.windows(2).any(|bytes| bytes == b"\x1b["),
                "{label} {args:?} emitted ANSI styling: {}",
                String::from_utf8_lossy(&output.stdout)
            );
            assert_eq!(
                output.stdout, baseline[index],
                "{label} must not alter text-mode data"
            );
        }
    }
}

#[test]
fn capabilities_publish_reproducibility_and_terminal_conventions() {
    let capabilities = run_json(&["--json", "capabilities"], &[]);
    let conventions = capabilities["conventions"]
        .as_array()
        .expect("capabilities conventions");
    for expected in [DETERMINISM_CONVENTION, TEXT_CONVENTION] {
        assert!(
            conventions.iter().any(|value| value == expected),
            "missing capability convention {expected:?}: {conventions:?}"
        );
    }
}
