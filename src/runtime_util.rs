use std::fs;
use std::path::Path;

use crate::{CliError, CliResult};

pub(crate) fn current_utc_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs.div_euclid(86_400);
    let seconds_of_day = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2).div_euclid(153);
    let day = doy - (153 * mp + 2).div_euclid(5) + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

pub(crate) fn chrono_like_counter() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

pub(crate) fn package_mutation_temp_path(file: &str, label: &str) -> String {
    let parent = Path::new(file)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let ext = Path::new(file)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(".{value}"))
        .unwrap_or_else(|| ".tmp".to_string());
    parent
        .join(format!(
            ".ooxml-rust-{label}-{}-{}{}",
            std::process::id(),
            chrono_like_counter(),
            ext
        ))
        .to_string_lossy()
        .to_string()
}

/// Resolve a caller-owned, destination-local stage for a package mutation.
///
/// A distinct `--out` is never used as the write path. This keeps any
/// pre-existing destination untouched until validation and operation-specific
/// readback have both succeeded, while retaining same-filesystem rename where
/// the filesystem permits it.
pub(crate) fn mutation_staging_path(file: &str, out: Option<&str>, label: &str) -> String {
    let anchor = out.filter(|value| !value.trim().is_empty()).unwrap_or(file);
    package_mutation_temp_path(anchor, label)
}

/// Publish or discard a validated caller-owned mutation stage.
pub(crate) fn finish_mutation_output(
    file: &str,
    staged_path: &str,
    out: Option<&str>,
    in_place: bool,
    backup: Option<&str>,
    dry_run: bool,
) -> CliResult<()> {
    if dry_run {
        let _ = fs::remove_file(staged_path);
        return Ok(());
    }
    let destination = if in_place {
        file
    } else {
        out.filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
    };
    if destination == file
        && let Some(backup_path) = backup.filter(|value| !value.trim().is_empty())
    {
        fs::copy(file, backup_path)
            .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
    }
    promote_mutation_stage(staged_path, destination)
}

fn promote_mutation_stage(staged_path: &str, destination: &str) -> CliResult<()> {
    match fs::rename(staged_path, destination) {
        Ok(()) => return Ok(()),
        Err(initial_error) if !Path::new(destination).exists() => {
            return Err(CliError::unexpected(format!(
                "failed to publish output file: {initial_error}"
            )));
        }
        Err(_) => {}
    }

    // Windows does not replace an existing destination with `rename`. Move the
    // old value aside first, then either complete the same-directory rename or
    // restore the old value. This is recoverable even where atomic replacement
    // is unavailable; it deliberately avoids a partial-copy fallback.
    let rollback_path = package_mutation_temp_path(destination, "publish-rollback");
    fs::rename(destination, &rollback_path).map_err(|err| {
        CliError::unexpected(format!(
            "failed to preserve existing output before publish: {err}"
        ))
    })?;
    match fs::rename(staged_path, destination) {
        Ok(()) => {
            let _ = fs::remove_file(&rollback_path);
            Ok(())
        }
        Err(publish_error) => match fs::rename(&rollback_path, destination) {
            Ok(()) => Err(CliError::unexpected(format!(
                "failed to publish output file; previous output restored: {publish_error}"
            ))),
            Err(restore_error) => Err(CliError::unexpected(format!(
                "failed to publish output file ({publish_error}) and failed to restore previous output from {rollback_path}: {restore_error}"
            ))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ooxml-runtime-util-{label}-{}-{}",
            std::process::id(),
            chrono_like_counter()
        ));
        fs::create_dir(&dir).expect("create test directory");
        dir
    }

    #[test]
    fn staging_is_local_to_the_requested_destination() {
        let dir = test_dir("stage-parent");
        let input = dir.join("input.xlsx");
        let output = dir.join("nested").join("output.xlsx");
        fs::create_dir(output.parent().expect("output parent")).expect("create output parent");

        let staged = mutation_staging_path(
            input.to_str().expect("input path"),
            Some(output.to_str().expect("output path")),
            "test",
        );
        assert_eq!(Path::new(&staged).parent(), output.parent());

        fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[test]
    fn publish_replaces_output_without_changing_input() {
        let dir = test_dir("replace-output");
        let input = dir.join("input.xlsx");
        let output = dir.join("output.xlsx");
        fs::write(&input, b"input").expect("write input");
        fs::write(&output, b"previous").expect("write previous output");
        let staged = mutation_staging_path(
            input.to_str().expect("input path"),
            Some(output.to_str().expect("output path")),
            "test",
        );
        fs::write(&staged, b"candidate").expect("write candidate");

        finish_mutation_output(
            input.to_str().expect("input path"),
            &staged,
            Some(output.to_str().expect("output path")),
            false,
            None,
            false,
        )
        .expect("publish candidate");

        assert_eq!(fs::read(&input).expect("read input"), b"input");
        assert_eq!(fs::read(&output).expect("read output"), b"candidate");
        assert!(!Path::new(&staged).exists());
        fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[test]
    fn dry_run_discards_stage_and_preserves_output() {
        let dir = test_dir("dry-run");
        let input = dir.join("input.xlsx");
        let output = dir.join("output.xlsx");
        fs::write(&input, b"input").expect("write input");
        fs::write(&output, b"previous").expect("write previous output");
        let staged = mutation_staging_path(
            input.to_str().expect("input path"),
            Some(output.to_str().expect("output path")),
            "test",
        );
        fs::write(&staged, b"candidate").expect("write candidate");

        finish_mutation_output(
            input.to_str().expect("input path"),
            &staged,
            Some(output.to_str().expect("output path")),
            false,
            None,
            true,
        )
        .expect("discard candidate");

        assert_eq!(fs::read(&output).expect("read output"), b"previous");
        assert!(!Path::new(&staged).exists());
        fs::remove_dir_all(dir).expect("remove test directory");
    }
}
