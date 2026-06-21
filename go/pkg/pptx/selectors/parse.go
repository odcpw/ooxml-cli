package selectors

import (
	"fmt"
	"strconv"
	"strings"
)

// Parse parses a selector string and returns the appropriate Selector type.
// It returns an error if the format is invalid.
//
// Supported formats:
//   - Placeholder key (bare word or colon-separated): "title", "body:0", "pic:1"
//   - Placeholder type: "@title", "@body"
//   - Placeholder index: "#0", "#3"
//   - Shape name: "~Name With Spaces"
//   - Shape ID: "shape:123"
//   - Slide number: "1", "5"
//   - Slide range: "1-3", "1,3,5-7"
//   - Wildcard selectors:
//   - "@*" or "@all-placeholders": all placeholders
//   - "@all-shapes": all shapes
//   - "@all-shapes-nonph": all non-placeholder shapes
//   - "@all-pictures": all pictures
//   - "@all-tables": all tables
func Parse(input string) (Selector, error) {
	if input == "" {
		return nil, fmt.Errorf("selector cannot be empty")
	}

	trimmed := strings.TrimSpace(input)

	// Check for prefix-based selectors
	if strings.HasPrefix(trimmed, "@") {
		// Wildcard and placeholder type selectors
		rest := strings.TrimSpace(trimmed[1:])
		if rest == "" {
			return nil, fmt.Errorf("selector cannot be empty after @")
		}

		// Check for wildcard selectors
		switch rest {
		case "*":
			return &WildcardAllPlaceholdersSelector{Format: "*"}, nil
		case "all-placeholders":
			return &WildcardAllPlaceholdersSelector{Format: "all-placeholders"}, nil
		case "all-shapes":
			return &WildcardAllShapesSelector{ExcludePlaceholders: false}, nil
		case "all-shapes-nonph":
			return &WildcardAllShapesSelector{ExcludePlaceholders: true}, nil
		case "all-pictures":
			return &WildcardAllPicturesSelector{}, nil
		case "all-tables":
			return &WildcardAllTablesSelector{}, nil
		default:
			// Regular placeholder type selector
			return &PlaceholderTypeSelector{Role: rest}, nil
		}
	}

	if strings.HasPrefix(trimmed, "#") {
		// Placeholder index selector
		indexStr := strings.TrimSpace(trimmed[1:])
		if indexStr == "" {
			return nil, fmt.Errorf("placeholder index selector cannot be empty after #")
		}
		idx, err := strconv.Atoi(indexStr)
		if err != nil {
			return nil, fmt.Errorf("invalid placeholder index: %w", err)
		}
		if idx < 0 {
			return nil, fmt.Errorf("placeholder index must be non-negative, got %d", idx)
		}
		return &PlaceholderIndexSelector{Index: idx}, nil
	}

	if strings.HasPrefix(trimmed, "~") {
		// Shape name selector
		name := trimmed[1:]
		if name == "" {
			return nil, fmt.Errorf("shape name selector cannot be empty after ~")
		}
		return &ShapeNameSelector{Name: name}, nil
	}

	if strings.HasPrefix(trimmed, "shape:") {
		// Shape ID selector
		idStr := strings.TrimSpace(trimmed[6:])
		if idStr == "" {
			return nil, fmt.Errorf("shape ID selector cannot be empty after 'shape:'")
		}
		id, err := strconv.Atoi(idStr)
		if err != nil {
			return nil, fmt.Errorf("invalid shape ID: %w", err)
		}
		if id < 0 {
			return nil, fmt.Errorf("shape ID must be non-negative, got %d", id)
		}
		return &ShapeIDSelector{ID: id}, nil
	}

	// Try parsing as slide number(s) or range(s)
	if isSlideSelector(trimmed) {
		return parseSlideSelector(trimmed)
	}

	// Default to placeholder key selector (bare word or key:index format)
	return &PlaceholderKeySelector{Key: trimmed}, nil
}

// isSlideSelector checks if the input looks like a slide selector (number(s) and/or ranges)
func isSlideSelector(input string) bool {
	// If it contains a comma, it's definitely a slide range
	if strings.Contains(input, ",") {
		return true
	}

	// If it contains a dash followed by a digit, it's a range
	if strings.Contains(input, "-") {
		// But check it's not at the start (could be negative sign or placeholder format)
		if !strings.HasPrefix(input, "-") {
			return true
		}
	}

	// If it's purely digits, it could be a slide number
	if isNumeric(input) {
		return true
	}

	return false
}

// isNumeric checks if a string contains only digits
func isNumeric(s string) bool {
	if s == "" {
		return false
	}
	for _, c := range s {
		if c < '0' || c > '9' {
			return false
		}
	}
	return true
}

// parseSlideSelector parses a slide selector string and returns the appropriate selector.
// Format: single number, comma-separated numbers, ranges with dashes, or combinations.
// Examples: "1", "1-3", "1,3,5-7"
func parseSlideSelector(input string) (Selector, error) {
	parts := strings.Split(input, ",")
	if len(parts) == 0 {
		return nil, fmt.Errorf("invalid slide selector")
	}

	if len(parts) == 1 && !strings.Contains(parts[0], "-") {
		// Single slide number
		num, err := strconv.Atoi(strings.TrimSpace(parts[0]))
		if err != nil {
			return nil, fmt.Errorf("invalid slide number: %w", err)
		}
		if num <= 0 {
			return nil, fmt.Errorf("slide number must be positive, got %d", num)
		}
		return &SlideNumberSelector{Number: num}, nil
	}

	// Parse as slide range
	var ranges []SlideRange
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}

		if strings.Contains(part, "-") {
			// Range format: "1-3"
			rangeParts := strings.Split(part, "-")
			if len(rangeParts) != 2 {
				return nil, fmt.Errorf("invalid slide range format: %s", part)
			}

			start, err := strconv.Atoi(strings.TrimSpace(rangeParts[0]))
			if err != nil {
				return nil, fmt.Errorf("invalid range start: %w", err)
			}
			if start <= 0 {
				return nil, fmt.Errorf("slide number must be positive, got %d", start)
			}

			end, err := strconv.Atoi(strings.TrimSpace(rangeParts[1]))
			if err != nil {
				return nil, fmt.Errorf("invalid range end: %w", err)
			}
			if end <= 0 {
				return nil, fmt.Errorf("slide number must be positive, got %d", end)
			}

			if start > end {
				return nil, fmt.Errorf("invalid range: start (%d) cannot be greater than end (%d)", start, end)
			}

			ranges = append(ranges, SlideRange{Start: start, End: end})
		} else {
			// Single slide number
			num, err := strconv.Atoi(part)
			if err != nil {
				return nil, fmt.Errorf("invalid slide number: %w", err)
			}
			if num <= 0 {
				return nil, fmt.Errorf("slide number must be positive, got %d", num)
			}
			ranges = append(ranges, SlideRange{Start: num, End: num})
		}
	}

	if len(ranges) == 0 {
		return nil, fmt.Errorf("invalid slide selector")
	}

	return &SlideRangeSelector{Ranges: ranges}, nil
}
