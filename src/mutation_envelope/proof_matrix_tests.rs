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
