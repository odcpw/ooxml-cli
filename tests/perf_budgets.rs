use serde_json::{Value, json};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const PERF_ENV: &str = "OOXML_PERF_BUDGETS";
const BASELINES_JSON: &str = include_str!("../testdata/golden/perf-budgets.json");
const PPTX_SLIDES: usize = 50;
const XLSX_ROWS: usize = 10_000;
const XLSX_COLS: usize = 10;
const XLSX_CELLS: usize = XLSX_ROWS * XLSX_COLS;
const CELL_PAYLOAD_BYTES: usize = 500;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct Budget {
    name: &'static str,
    baseline_ms: u64,
    hard_limit_ms: u64,
    max_rss_mib: Option<u64>,
}

#[derive(Debug)]
struct Measurement {
    status: ExitStatus,
    stderr: Vec<u8>,
    elapsed: Duration,
    peak_rss_kib: Option<u64>,
}

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ooxml-perf-{label}-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create performance test directory");
        Self(path)
    }

    fn join(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn performance_baseline_contract_pins_all_release_workloads() {
    let baselines = baseline_document();
    assert_eq!(baselines["schemaVersion"], 1);
    assert_eq!(baselines["maxRegressionPercent"], 25);
    assert_eq!(
        baselines["measurements"]
            .as_object()
            .expect("measurements object")
            .len(),
        4
    );

    for name in [
        "pptxBuild50Slides",
        "xlsxRangesSet100kCells",
        "outlineLargestCommittedWorkbook",
        "checkLargestCommittedWorkbook",
    ] {
        let budget = budget(name);
        assert!(budget.baseline_ms > 0, "{name} baseline must be positive");
        assert!(
            budget.hard_limit_ms >= budget.baseline_ms,
            "{name} hard limit must not be below its baseline"
        );
        assert_eq!(
            regression_limit_ms(&budget),
            budget.hard_limit_ms,
            "{name} pins its 25% regression threshold to the documented hard limit"
        );
    }

    assert_eq!(budget("xlsxRangesSet100kCells").max_rss_mib, Some(300));
}

#[test]
fn streamed_json_file_range_set_matches_inline_output_bytes() {
    let temp = TestDir::new("xlsx-streaming-parity");
    let values_file = temp.join("values.json");
    let file_output = temp.join("file.xlsx");
    let inline_output = temp.join("inline.xlsx");
    let values =
        r#"[[null,"text",1.0,true,{"formula":"SUM(C1:C1)"},{"value":"42","type":"number"}]]"#;
    fs::write(&values_file, values).expect("write parity values JSON");

    let common = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "Sheet1",
        "--range",
        "A1:F1",
    ];
    let mut file_args = owned_args(&common);
    file_args.extend(owned_args(&[
        "--values-file",
        path_text(&values_file),
        "--data-format",
        "json",
        "--out",
        path_text(&file_output),
    ]));
    run_ooxml_ok(&file_args, "streamed file range set");

    let mut inline_args = owned_args(&common);
    inline_args.extend(owned_args(&[
        "--values",
        values,
        "--out",
        path_text(&inline_output),
    ]));
    run_ooxml_ok(&inline_args, "inline range set");

    assert_eq!(
        fs::read(&file_output).expect("read streamed output"),
        fs::read(&inline_output).expect("read inline output"),
        "streaming the JSON file must not change produced XLSX bytes"
    );
    assert_strict_valid(&file_output);
    assert_strict_valid(&inline_output);
}

#[test]
fn pptx_build_50_slides_stays_within_release_budget() {
    if !release_perf_enabled() {
        return;
    }
    measure_pptx_build_50_slides();
}

#[test]
fn xlsx_ranges_set_100k_cells_from_50mb_json_stays_within_release_budgets() {
    if !release_perf_enabled() {
        return;
    }
    measure_xlsx_ranges_set_100k_cells();
}

#[test]
fn outline_largest_committed_workbook_stays_within_release_budget() {
    if !release_perf_enabled() {
        return;
    }
    measure_outline_largest_committed_workbook();
}

#[test]
fn check_largest_committed_workbook_without_render_stays_within_release_budget() {
    if !release_perf_enabled() {
        return;
    }
    measure_check_largest_committed_workbook();
}

fn measure_pptx_build_50_slides() {
    let temp = TestDir::new("pptx-build-50");
    let spec = temp.join("deck.json");
    let output = temp.join("deck.pptx");
    write_pptx_spec(&spec);

    let args = owned_args(&[
        "--json",
        "pptx",
        "build",
        "--spec",
        path_text(&spec),
        "--out",
        path_text(&output),
    ]);
    let measurement = measure_ooxml(&args);
    assert_measurement_ok(&measurement, "50-slide PPTX build");
    assert_budget(&budget("pptxBuild50Slides"), &measurement);
    assert_strict_valid(&output);
}

fn measure_xlsx_ranges_set_100k_cells() {
    let temp = TestDir::new("xlsx-ranges-100k");
    let input = temp.join("input.xlsx");
    let values = temp.join("values.json");
    let output = temp.join("output.xlsx");
    write_large_values(&values);

    let scaffold = owned_args(&["--json", "xlsx", "scaffold", path_text(&input), "--force"]);
    run_ooxml_ok(&scaffold, "XLSX performance scaffold");

    let args = owned_args(&[
        "--json",
        "xlsx",
        "ranges",
        "set",
        path_text(&input),
        "--sheet",
        "Sheet1",
        "--range",
        "A1:J10000",
        "--values-file",
        path_text(&values),
        "--data-format",
        "json",
        "--max-cells",
        "100000",
        "--out",
        path_text(&output),
    ]);
    let measurement = measure_ooxml(&args);
    assert_measurement_ok(&measurement, "100,000-cell XLSX ranges set");
    assert_budget(&budget("xlsxRangesSet100kCells"), &measurement);
    assert_strict_valid(&output);
}

fn measure_outline_largest_committed_workbook() {
    let workbook = largest_committed_workbook();
    let args = owned_args(&["--json", "outline", path_text(&workbook)]);
    run_ooxml_ok(&args, "outline warm-up");

    let measurement = measure_ooxml(&args);
    assert_measurement_ok(&measurement, "outline largest committed workbook");
    assert_budget(&budget("outlineLargestCommittedWorkbook"), &measurement);
}

fn measure_check_largest_committed_workbook() {
    let workbook = largest_committed_workbook();
    let args = owned_args(&[
        "--json",
        "check",
        path_text(&workbook),
        "--openxml-sdk",
        "skip",
    ]);
    run_ooxml_ok(&args, "check warm-up");

    let measurement = measure_ooxml(&args);
    assert_measurement_ok(&measurement, "check largest committed workbook");
    assert_budget(&budget("checkLargestCommittedWorkbook"), &measurement);
}

fn baseline_document() -> Value {
    serde_json::from_str(BASELINES_JSON).expect("valid performance baseline JSON")
}

fn budget(name: &'static str) -> Budget {
    let document = baseline_document();
    let entry = document["measurements"]
        .get(name)
        .unwrap_or_else(|| panic!("missing performance baseline {name}"));
    Budget {
        name,
        baseline_ms: entry["baselineMs"].as_u64().expect("baselineMs"),
        hard_limit_ms: entry["hardLimitMs"].as_u64().expect("hardLimitMs"),
        max_rss_mib: entry.get("maxRssMiB").and_then(Value::as_u64),
    }
}

fn regression_limit_ms(budget: &Budget) -> u64 {
    let percent = baseline_document()["maxRegressionPercent"]
        .as_u64()
        .expect("maxRegressionPercent");
    budget
        .baseline_ms
        .saturating_mul(100 + percent)
        .div_ceil(100)
}

fn assert_budget(budget: &Budget, measurement: &Measurement) {
    let elapsed_ms = measurement.elapsed.as_millis() as u64;
    let regression_limit = regression_limit_ms(budget);
    let peak_rss_mib = measurement.peak_rss_kib.map(|value| value.div_ceil(1024));
    eprintln!(
        "PERF_BUDGET name={} elapsed_ms={} baseline_ms={} regression_limit_ms={} hard_limit_ms={} peak_rss_mib={}",
        budget.name,
        elapsed_ms,
        budget.baseline_ms,
        regression_limit,
        budget.hard_limit_ms,
        peak_rss_mib
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unavailable".to_string())
    );
    assert!(
        elapsed_ms <= regression_limit,
        "{} took {} ms, above its stored baseline plus 25% ({} ms)",
        budget.name,
        elapsed_ms,
        regression_limit
    );
    assert!(
        elapsed_ms <= budget.hard_limit_ms,
        "{} took {} ms, above its hard budget of {} ms",
        budget.name,
        elapsed_ms,
        budget.hard_limit_ms
    );
    if let Some(max_rss_mib) = budget.max_rss_mib {
        #[cfg(target_os = "linux")]
        assert!(
            peak_rss_mib.is_some(),
            "{} must report Linux peak RSS",
            budget.name
        );
        if let Some(actual) = peak_rss_mib {
            assert!(
                actual <= max_rss_mib,
                "{} used {} MiB peak RSS, above its {} MiB budget",
                budget.name,
                actual,
                max_rss_mib
            );
        }
    }
}

fn perf_budgets_enabled() -> bool {
    std::env::var(PERF_ENV).as_deref() == Ok("1")
}

fn release_perf_enabled() -> bool {
    if !perf_budgets_enabled() {
        eprintln!(
            "skipping release performance workload; set {PERF_ENV}=1 and run cargo test --release --test perf_budgets -- --nocapture --test-threads=1"
        );
        return false;
    }
    assert!(
        !cfg!(debug_assertions),
        "{PERF_ENV}=1 requires cargo test --release so debug builds do not create misleading baselines"
    );
    true
}

fn measure_ooxml(args: &[OsString]) -> Measurement {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ooxml performance workload");
    let started = Instant::now();
    let mut peak_rss_kib = None;
    loop {
        #[cfg(target_os = "linux")]
        {
            peak_rss_kib = peak_rss_kib.max(linux_resident_kib(child.id()));
        }
        if child.try_wait().expect("poll ooxml workload").is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let elapsed = started.elapsed();
    let output = child
        .wait_with_output()
        .expect("collect ooxml workload output");
    Measurement {
        status: output.status,
        stderr: output.stderr,
        elapsed,
        peak_rss_kib,
    }
}

#[cfg(target_os = "linux")]
fn linux_resident_kib(pid: u32) -> Option<u64> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for field in ["VmHWM:", "VmRSS:"] {
        if let Some(value) = status.lines().find_map(|line| {
            line.strip_prefix(field)
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|value| value.parse::<u64>().ok())
        }) {
            return Some(value);
        }
    }
    None
}

fn run_ooxml_ok(args: &[OsString], label: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|error| panic!("run {label}: {error}"));
    assert!(
        output.status.success(),
        "{label} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_measurement_ok(measurement: &Measurement, label: &str) {
    assert!(
        measurement.status.success(),
        "{label} failed: {}",
        String::from_utf8_lossy(&measurement.stderr)
    );
}

fn assert_strict_valid(package: &Path) {
    let args = owned_args(&["--json", "validate", "--strict", path_text(package)]);
    run_ooxml_ok(&args, "strict validation of performance output");
}

fn owned_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("UTF-8 performance fixture path")
}

fn write_pptx_spec(path: &Path) {
    let slides = (1..=PPTX_SLIDES)
        .map(|number| {
            json!({
                "id": format!("slide-{number:02}"),
                "layout": "Title Only",
                "title": format!("Performance slide {number} of {PPTX_SLIDES}")
            })
        })
        .collect::<Vec<_>>();
    let spec = json!({
        "schemaVersion": 1,
        "family": "pptx",
        "theme": "neutral",
        "size": "16:9",
        "slides": slides
    });
    let mut output = BufWriter::new(File::create(path).expect("create PPTX spec"));
    serde_json::to_writer(&mut output, &spec).expect("write PPTX spec");
    output.flush().expect("flush PPTX spec");
}

fn write_large_values(path: &Path) {
    let mut output = BufWriter::new(File::create(path).expect("create values JSON"));
    let filler = "x".repeat(CELL_PAYLOAD_BYTES);
    output.write_all(b"[").expect("start values JSON");
    for row in 0..XLSX_ROWS {
        if row > 0 {
            output.write_all(b",").expect("separate rows");
        }
        output.write_all(b"[").expect("start row");
        for column in 0..XLSX_COLS {
            if column > 0 {
                output.write_all(b",").expect("separate cells");
            }
            let prefix = format!("r{row:05}c{column:02}-");
            output.write_all(b"\"").expect("start cell");
            output
                .write_all(prefix.as_bytes())
                .expect("write cell prefix");
            output
                .write_all(&filler.as_bytes()[..CELL_PAYLOAD_BYTES - prefix.len()])
                .expect("write cell payload");
            output.write_all(b"\"").expect("end cell");
        }
        output.write_all(b"]").expect("end row");
    }
    output.write_all(b"]").expect("end values JSON");
    output.flush().expect("flush values JSON");

    assert_eq!(XLSX_CELLS, 100_000);
    assert!(
        fs::metadata(path).expect("values JSON metadata").len() >= 50_000_000,
        "ranges-set workload must remain at least 50 MB"
    );
}

fn largest_committed_workbook() -> PathBuf {
    fn visit(directory: &Path, largest: &mut Option<(u64, PathBuf)>) {
        let mut entries = fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
            .collect::<Result<Vec<_>, _>>()
            .expect("read fixture directory entries");
        entries.sort_by_key(std::fs::DirEntry::path);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, largest);
            } else if path.extension().and_then(|value| value.to_str()) == Some("xlsx") {
                let size = entry.metadata().expect("workbook metadata").len();
                if largest.as_ref().is_none_or(|(current, _)| size > *current) {
                    *largest = Some((size, path));
                }
            }
        }
    }

    let mut largest = None;
    visit(Path::new("testdata"), &mut largest);
    let (bytes, path) = largest.expect("at least one committed XLSX fixture");
    eprintln!(
        "PERF_FIXTURE largest_committed_workbook={} bytes={bytes}",
        path.display()
    );
    path
}
