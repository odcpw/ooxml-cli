package cli

import (
	"fmt"
	"strings"
)

// SelectorCandidate is a paste-able selector suggestion offered to an agent when a
// selector fails to resolve. Primary is the canonical selector (always present and
// paste-able); Selectors holds the full alias list used for substring matching.
type SelectorCandidate struct {
	Primary   string
	Selectors []string
}

// maxSelectorCandidates caps how many "did you mean" suggestions we surface so the
// error message stays readable for agents.
const maxSelectorCandidates = 3

// BuildSelectorCandidates returns up to maxCount paste-able primary selectors that an
// agent could try instead of a missed selector. It prefers items whose selectors (or
// primary) contain the missed token (case-insensitive substring match); if nothing
// matches it falls back to the first items in catalog order so "did you mean" is always
// useful when the catalog is non-empty. The function is pure (no CLIError dependency)
// to keep it table-testable.
func BuildSelectorCandidates(items []SelectorCandidate, selector string, maxCount int) []string {
	if maxCount <= 0 {
		maxCount = maxSelectorCandidates
	}
	needle := strings.ToLower(strings.TrimSpace(selector))

	seen := map[string]bool{}
	out := make([]string, 0, maxCount)
	add := func(primary string) bool {
		primary = strings.TrimSpace(primary)
		if primary == "" || seen[primary] {
			return false
		}
		seen[primary] = true
		out = append(out, primary)
		return len(out) >= maxCount
	}

	if needle != "" {
		for _, item := range items {
			if selectorContains(item, needle) {
				if add(item.Primary) {
					return out
				}
			}
		}
	}

	// Prefer substring matches; only fall back to first-N catalog order when the
	// miss matched nothing, so "did you mean" is always useful for a non-empty catalog.
	if len(out) > 0 {
		return out
	}
	for _, item := range items {
		if add(item.Primary) {
			break
		}
	}
	return out
}

func selectorContains(item SelectorCandidate, needle string) bool {
	if strings.Contains(strings.ToLower(item.Primary), needle) {
		return true
	}
	for _, sel := range item.Selectors {
		if strings.Contains(strings.ToLower(sel), needle) {
			return true
		}
	}
	return false
}

// SelectorNotFoundError builds a TargetNotFound CLIError whose message lists nearby
// valid candidates and/or a discovery command, e.g.:
//
//	sheet not found: foo; did you mean: sheetId:1, sheetId:2; discover with `ooxml --json xlsx sheets list <file>`
//
// entity is the noun ("sheet", "chart", ...). candidates and discoveryCmd are both
// optional; whichever are present are appended. The exit code stays ExitTargetNotFound
// so existing contracts are unchanged.
func SelectorNotFoundError(entity, selector string, candidates []string, discoveryCmd string) *CLIError {
	var b strings.Builder
	fmt.Fprintf(&b, "%s not found: %s", entity, selector)
	if len(candidates) > 0 {
		fmt.Fprintf(&b, "; did you mean: %s", strings.Join(candidates, ", "))
	}
	if strings.TrimSpace(discoveryCmd) != "" {
		fmt.Fprintf(&b, "; discover with `%s`", strings.TrimSpace(discoveryCmd))
	}
	return NewCLIError(ExitTargetNotFound, b.String())
}
