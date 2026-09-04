use std::path::Path;

pub(super) fn path_prefix_aliases(path: &Path) -> Vec<String> {
    let mut aliases = vec![path.to_string_lossy().into_owned()];
    if let Ok(canonical) = path.canonicalize() {
        aliases.push(canonical.to_string_lossy().into_owned());
    }
    aliases.sort_by_key(|alias| std::cmp::Reverse(alias.len()));
    aliases.dedup();
    aliases
}

pub(super) fn scrub_path_string(text: &str, prefix: &str, replacement: &str) -> String {
    let native = prefix.replace('/', "\\");
    let slashed = prefix.replace('\\', "/");
    let mut path_variants = vec![prefix.to_string(), native.clone(), slashed.clone()];
    if native.as_bytes().get(1) == Some(&b':') {
        path_variants.push(format!(r"\\?\{native}"));
        path_variants.push(format!("//?/{slashed}"));
    }
    path_variants.sort();
    path_variants.dedup();

    let mut replacements = Vec::new();
    for variant in path_variants {
        let escaped = variant.replace('\\', r"\\");
        replacements.push(format!("'{escaped}'"));
        replacements.push(format!("\"{escaped}\""));
        replacements.push(escaped);
        replacements.push(crate::command_arg(&variant));
        replacements.push(format!("'{variant}'"));
        replacements.push(format!("\"{variant}\""));
        replacements.push(variant);
    }
    replacements.sort_by_key(|value| std::cmp::Reverse(value.len()));
    replacements.dedup();

    replacements
        .into_iter()
        .fold(text.to_string(), |text, from| {
            text.replace(&from, replacement)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrubber_handles_windows_native_quoted_escaped_and_verbatim_paths() {
        let prefix = r"C:\Users\RUNNER~1\AppData\Local\Temp\ooxml-docx-build-123";
        let escaped = prefix.replace('\\', r"\\");
        for text in [
            format!(r"ooxml validate --strict {prefix}\document.docx"),
            format!("\"{prefix}\""),
            format!(r"\\?\{prefix}\operations.json"),
            format!("{}/document.docx", prefix.replace('\\', "/")),
            format!(r#"{{"file":"{escaped}\\generated\\report.docx"}}"#),
        ] {
            let scrubbed = scrub_path_string(&text, prefix, "<build-stage>");
            assert!(
                !scrubbed.contains("RUNNER~1"),
                "unscrubbed path: {scrubbed}"
            );
            assert!(
                scrubbed.contains("<build-stage>"),
                "missing token: {scrubbed}"
            );
        }
    }
}
