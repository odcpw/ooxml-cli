pub(super) fn optional_cfb_stream_path(paths: &[String], want: &str) -> Option<String> {
    let path = find_cfb_stream_path(paths, want);
    (!path.is_empty()).then_some(path)
}

pub(super) fn find_cfb_stream_path(paths: &[String], want: &str) -> String {
    paths
        .iter()
        .find(|path| path.replace('\\', "/").eq_ignore_ascii_case(want))
        .cloned()
        .unwrap_or_default()
}
