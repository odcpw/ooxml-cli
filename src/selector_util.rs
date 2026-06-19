pub(crate) fn selector_candidates(
    items: &[(&str, &[String])],
    selector: &str,
    max_count: usize,
) -> Vec<String> {
    let needle = selector.trim().to_ascii_lowercase();
    let mut seen = Vec::<String>::new();
    if !needle.is_empty() {
        for (primary, selectors) in items {
            let matched = primary.to_ascii_lowercase().contains(&needle)
                || selectors
                    .iter()
                    .any(|selector| selector.to_ascii_lowercase().contains(&needle));
            if matched && push_selector_candidate(&mut seen, primary, max_count) {
                return seen;
            }
        }
    }
    if !seen.is_empty() {
        return seen;
    }
    for (primary, _) in items {
        if push_selector_candidate(&mut seen, primary, max_count) {
            break;
        }
    }
    seen
}

fn push_selector_candidate(seen: &mut Vec<String>, primary: &str, max_count: usize) -> bool {
    let primary = primary.trim();
    if primary.is_empty() || seen.iter().any(|existing| existing == primary) {
        return false;
    }
    seen.push(primary.to_string());
    seen.len() >= max_count
}

pub(crate) fn add_selector(selectors: &mut Vec<String>, selector: String) {
    if selector.trim().is_empty() || selectors.iter().any(|existing| existing == &selector) {
        return;
    }
    selectors.push(selector);
}
