use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::Value;

use super::{
    DOCX_MUTATION_COMMANDS, PACKAGE_MUTATION_COMMANDS, PPTX_MUTATION_COMMANDS,
    XLSX_MUTATION_COMMANDS,
};

const LINUX_TIERS: [&str; 4] = ["structural", "readback", "validate", "conformance"];
const EVIDENCE_GROUPS: [(&str, usize); 4] =
    [("docx", 27), ("xlsx", 60), ("pptx", 60), ("package", 5)];

fn mutation_inventory() -> BTreeSet<String> {
    DOCX_MUTATION_COMMANDS
        .iter()
        .chain(XLSX_MUTATION_COMMANDS)
        .chain(PPTX_MUTATION_COMMANDS)
        .chain(PACKAGE_MUTATION_COMMANDS)
        .map(|spec| format!("ooxml {}", spec.path.join(" ")))
        .collect()
}

fn evidence_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("golden")
        .join("artifact-proof-matrix")
}

fn load_contract_evidence() -> BTreeMap<String, Value> {
    let mut proofs = BTreeMap::new();
    for (group, expected_count) in EVIDENCE_GROUPS {
        let path = evidence_root().join(format!("{group}-contract-evidence.json"));
        let bytes = fs::read(&path)
            .unwrap_or_else(|error| panic!("read contract evidence {}: {error}", path.display()));
        let document: Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|error| panic!("parse contract evidence {}: {error}", path.display()));
        assert_eq!(
            document["schemaVersion"],
            "ooxml-cli.mutation-contract-evidence.v1",
            "{} schema version",
            path.display()
        );
        let rows = document["proofs"]
            .as_array()
            .unwrap_or_else(|| panic!("{} proofs array", path.display()));
        assert_eq!(rows.len(), expected_count, "{} row count", path.display());
        for row in rows {
            let command = row["commandPath"]
                .as_str()
                .unwrap_or_else(|| panic!("{} commandPath", path.display()))
                .to_string();
            assert!(
                proofs.insert(command.clone(), row.clone()).is_none(),
                "duplicate proof row for {command}"
            );
        }
    }
    proofs
}

#[test]
fn linux_proof_matrix_has_zero_gaps_for_all_152_mutations() {
    let inventory = mutation_inventory();
    assert_eq!(inventory.len(), 152, "reviewed mutation denominator");

    let manifest_paths = crate::command_manifest::capability_commands()
        .into_iter()
        .filter_map(|command| command["path"].as_str().map(str::to_string))
        .collect::<BTreeSet<_>>();
    let missing_manifest = inventory
        .difference(&manifest_paths)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing_manifest.is_empty(),
        "mutation inventory commands absent from CommandSpec manifest: {missing_manifest:?}"
    );

    let proofs = load_contract_evidence();
    let proof_paths = proofs.keys().cloned().collect::<BTreeSet<_>>();
    let missing_rows = inventory
        .difference(&proof_paths)
        .cloned()
        .collect::<Vec<_>>();
    let unknown_rows = proof_paths
        .difference(&inventory)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing_rows.is_empty() && unknown_rows.is_empty(),
        "proof row inventory mismatch; missing={missing_rows:?}; unknown={unknown_rows:?}"
    );

    let mut gaps = Vec::new();
    let mut matrix = String::from("command | structural | readback | validate | conformance\n");
    for command in &inventory {
        let proof = &proofs[command];
        let statuses =
            LINUX_TIERS.map(|tier| proof["tiers"][tier]["status"].as_str().unwrap_or("missing"));
        for (tier, status) in LINUX_TIERS.into_iter().zip(statuses) {
            if status != "passed" {
                gaps.push(format!("{command}:{tier}={status}"));
            }
        }
        matrix.push_str(&format!(
            "{command} | {} | {} | {} | {}\n",
            statuses[0], statuses[1], statuses[2], statuses[3]
        ));
    }
    assert!(
        gaps.is_empty(),
        "Linux mutation proof gaps ({} of {}×{} cells): {gaps:?}\n{matrix}",
        gaps.len(),
        inventory.len(),
        LINUX_TIERS.len()
    );
}

#[test]
fn ci_runs_the_zero_gap_linux_and_windows_contract_lanes() {
    let ci = include_str!("../../.github/workflows/ci.yml");
    assert!(ci.contains("linux_proof_matrix_has_zero_gaps_for_all_152_mutations"));
    assert!(ci.contains("OOXML_CONTRACT_PROOF_DIR"));
    assert!(ci.contains("cargo test --test mutation_envelope"));
    assert!(ci.contains("-ContractEvidenceDir $proofEvidenceDir"));
    assert!(ci.contains("-FailOnArtifactProofGap"));

    let smoke = include_str!("../../tools/windows-office-edit-smoke.ps1");
    assert!(smoke.contains("contract mutation paths: 152"));
    assert!(smoke.contains("-SkipOfficeRequirement"));
}

#[test]
fn powershell_tools_brace_variables_before_literal_colons_in_double_quotes() {
    let calibration = r##"
Write-Host "bad $path: value"
Write-Host "good ${path}: value and scoped $env:TEMP"
Write-Host 'literal $other: value'
# "comment $ignored: value"
"##;
    let calibration_hazards = unbraced_variable_colons_in_double_quotes(calibration);
    assert_eq!(calibration_hazards.len(), 1, "scanner calibration");
    assert_eq!(
        &calibration[calibration_hazards[0]..calibration_hazards[0] + 6],
        "$path:"
    );

    let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools");
    let mut scripts = fs::read_dir(&tools)
        .expect("read tools directory")
        .map(|entry| entry.expect("read tools entry").path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("ps1"))
        .collect::<Vec<_>>();
    scripts.sort();
    assert!(!scripts.is_empty(), "expected committed PowerShell tools");

    let mut hazards = Vec::new();
    for path in scripts {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read PowerShell tool {}: {error}", path.display()));
        for offset in unbraced_variable_colons_in_double_quotes(&source) {
            let line = source[..offset]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            hazards.push(format!("{}:{line}", path.display()));
        }
    }
    assert!(
        hazards.is_empty(),
        "PowerShell parses `$name:` as a scoped variable inside double-quoted strings; use `${{name}}:` at {hazards:?}"
    );
}

#[test]
fn powershell_generic_lists_are_materialized_before_array_binding() {
    let smoke = include_str!("../../tools/windows-office-edit-smoke.ps1");
    assert!(smoke.contains("[object[]]$additionalScenarios = $additional.ToArray()"));
    assert!(smoke.contains("[object[]]$contractScenarios = @(Import-ContractEvidenceScenarios"));
    assert!(!smoke.contains("return @($additional)"));

    let matrix = include_str!("../../tools/artifact-proof-matrix.ps1");
    assert!(matrix.contains("return [object[]]$items.ToArray()"));
    assert_eq!(
        matrix
            .matches("evidence = [string[]]$evidence.ToArray()")
            .count(),
        2,
        "both generic string-list evidence paths must materialize native arrays"
    );
}

fn unbraced_variable_colons_in_double_quotes(source: &str) -> Vec<usize> {
    const VALID_SCOPES: &[&str] = &["env", "global", "local", "private", "script", "using"];
    let bytes = source.as_bytes();
    let mut hazards = Vec::new();
    let mut index = 0usize;
    let mut in_double = false;
    let mut in_single = false;
    let mut line_comment = false;
    let mut block_comment = false;
    while index < bytes.len() {
        if line_comment {
            if bytes[index] == b'\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_comment {
            if bytes[index..].starts_with(b"#>") {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if !in_double && !in_single && bytes[index..].starts_with(b"<#") {
            block_comment = true;
            index += 2;
            continue;
        }
        if !in_double && !in_single && bytes[index] == b'#' {
            line_comment = true;
            index += 1;
            continue;
        }
        if bytes[index] == b'`' && in_double {
            index = (index + 2).min(bytes.len());
            continue;
        }
        if bytes[index] == b'\'' && !in_double {
            if in_single && bytes.get(index + 1) == Some(&b'\'') {
                index += 2;
                continue;
            }
            in_single = !in_single;
            index += 1;
            continue;
        }
        if bytes[index] == b'"' && !in_single {
            in_double = !in_double;
            index += 1;
            continue;
        }
        if in_double && bytes[index] == b'$' && bytes.get(index + 1) != Some(&b'{') {
            let name_start = index + 1;
            let mut end = name_start;
            while bytes
                .get(end)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
                end += 1;
            }
            if end > name_start && bytes.get(end) == Some(&b':') {
                let name = &source[name_start..end];
                if !VALID_SCOPES.contains(&name.to_ascii_lowercase().as_str()) {
                    hazards.push(index);
                }
            }
        }
        index += 1;
    }
    hazards
}
