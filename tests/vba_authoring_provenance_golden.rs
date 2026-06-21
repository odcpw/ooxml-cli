use sha2::{Digest, Sha256};

struct Fixture {
    name: &'static str,
    family: &'static str,
    provenance: &'static str,
    bin: &'static [u8],
    expected_size: usize,
    expected_sha256: &'static str,
    sources: &'static [&'static str],
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "docx-class",
        family: "docx",
        provenance: include_str!("../testdata/golden/vba-authoring/docx-class/PROVENANCE.md"),
        bin: include_bytes!("../testdata/golden/vba-authoring/docx-class/vbaProject.bin"),
        expected_size: 6144,
        expected_sha256: "9a0d1e425908a52909d472e794640dec13fd27d56f8b6588a3609d0420070aec",
        sources: &["AgentDoc.bas", "Worker.cls"],
    },
    Fixture {
        name: "docx-standard",
        family: "docx",
        provenance: include_str!("../testdata/golden/vba-authoring/docx-standard/PROVENANCE.md"),
        bin: include_bytes!("../testdata/golden/vba-authoring/docx-standard/vbaProject.bin"),
        expected_size: 5120,
        expected_sha256: "d372fcdb4a7e43352242b92c67f348a630a75247087f689357537476f15502a3",
        sources: &["AgentDoc.bas"],
    },
    Fixture {
        name: "pptx-class",
        family: "pptx",
        provenance: include_str!("../testdata/golden/vba-authoring/pptx-class/PROVENANCE.md"),
        bin: include_bytes!("../testdata/golden/vba-authoring/pptx-class/vbaProject.bin"),
        expected_size: 5120,
        expected_sha256: "417f50943286b0a7e4d01afbc7a659970bc42c586ecd9843122b4bff33ea03ea",
        sources: &["AgentSlide.bas", "Worker.cls"],
    },
    Fixture {
        name: "pptx-standard",
        family: "pptx",
        provenance: include_str!("../testdata/golden/vba-authoring/pptx-standard/PROVENANCE.md"),
        bin: include_bytes!("../testdata/golden/vba-authoring/pptx-standard/vbaProject.bin"),
        expected_size: 4096,
        expected_sha256: "8752348bae9b3fd624c476431d706ddf03a95ddbdb24e47465ebf98a8a389d0f",
        sources: &["AgentSlide.bas"],
    },
    Fixture {
        name: "xlsx-class",
        family: "xlsx",
        provenance: include_str!("../testdata/golden/vba-authoring/xlsx-class/PROVENANCE.md"),
        bin: include_bytes!("../testdata/golden/vba-authoring/xlsx-class/vbaProject.bin"),
        expected_size: 6656,
        expected_sha256: "6afab85a97be6608d0bfdf011be599a2c4f1f018447788def5a289d9814f6172",
        sources: &["AgentSmoke.bas", "Worker.cls"],
    },
    Fixture {
        name: "xlsx-standard",
        family: "xlsx",
        provenance: include_str!("../testdata/golden/vba-authoring/xlsx-standard/PROVENANCE.md"),
        bin: include_bytes!("../testdata/golden/vba-authoring/xlsx-standard/vbaProject.bin"),
        expected_size: 4096,
        expected_sha256: "21479229375710ab564da290ba3e32f430a70ec1bbeaac9b4998a18037faf19c",
        sources: &["AgentSmoke.bas"],
    },
];

#[test]
fn generated_vba_project_goldens_record_provenance() {
    for fixture in FIXTURES {
        assert_eq!(
            fixture.bin.len(),
            fixture.expected_size,
            "{} vbaProject.bin size changed",
            fixture.name
        );
        assert_eq!(
            sha256_hex(fixture.bin),
            fixture.expected_sha256,
            "{} vbaProject.bin sha256 changed",
            fixture.name
        );

        assert_provenance_contains(fixture, "vba build-bin");
        assert_provenance_contains(fixture, "vbaProject.bin");
        assert_provenance_contains(fixture, "inspect-bin.json");
        assert_provenance_contains(fixture, &format!("--family {}", fixture.family));
        assert_provenance_contains(fixture, &format!("{} bytes", fixture.expected_size));
        assert_provenance_contains(fixture, fixture.expected_sha256);

        for source in fixture.sources {
            assert_provenance_contains(fixture, source);
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn assert_provenance_contains(fixture: &Fixture, needle: &str) {
    assert!(
        fixture.provenance.contains(needle),
        "{} PROVENANCE.md should mention {needle:?}",
        fixture.name
    );
}
