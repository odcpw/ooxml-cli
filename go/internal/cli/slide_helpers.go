package cli

import (
	"fmt"
	"sort"
	"strconv"
	"strings"
)

// parseSlideSpec parses a slide specification string into a list of slide numbers.
// Supports:
// - Single numbers: "1,2,3"
// - Ranges: "1-5" (inclusive)
// - Mixed: "1,3-5,7"
// Returns 1-indexed slide numbers
func parseSlideSpec(spec string) ([]int, error) {
	spec = strings.TrimSpace(spec)
	if spec == "" {
		return nil, fmt.Errorf("empty specification")
	}

	var result []int
	seenMap := make(map[int]bool)

	// Split by comma to get ranges and individual numbers
	parts := strings.Split(spec, ",")
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}

		// Check if it's a range (contains a dash)
		if strings.Contains(part, "-") {
			// Split by dash
			rangeParts := strings.Split(part, "-")
			if len(rangeParts) != 2 {
				return nil, fmt.Errorf("invalid range format: %s", part)
			}

			start, err := strconv.Atoi(strings.TrimSpace(rangeParts[0]))
			if err != nil || start <= 0 {
				return nil, fmt.Errorf("invalid range start: %s", rangeParts[0])
			}

			end, err := strconv.Atoi(strings.TrimSpace(rangeParts[1]))
			if err != nil || end <= 0 {
				return nil, fmt.Errorf("invalid range end: %s", rangeParts[1])
			}

			if start > end {
				return nil, fmt.Errorf("range start (%d) cannot be greater than end (%d)", start, end)
			}

			// Add all numbers in the range, avoiding duplicates
			for i := start; i <= end; i++ {
				if !seenMap[i] {
					result = append(result, i)
					seenMap[i] = true
				}
			}
		} else {
			// Single number
			num, err := strconv.Atoi(part)
			if err != nil || num <= 0 {
				return nil, fmt.Errorf("invalid slide number: %s", part)
			}

			// Add if not already included
			if !seenMap[num] {
				result = append(result, num)
				seenMap[num] = true
			}
		}
	}

	// Sort the result
	sort.Ints(result)

	return result, nil
}
