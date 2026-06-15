package opc

import (
	"strings"
)

// NormalizeURI normalizes an OPC URI to a canonical form.
// - Ensures leading /
// - Removes trailing / unless it's the root
// - Removes . and .. segments where possible
func NormalizeURI(uri string) string {
	if uri == "" {
		return "/"
	}
	uri = strings.ReplaceAll(uri, "\\", "/")

	// Ensure leading /
	if uri[0] != '/' {
		uri = "/" + uri
	}

	// Split and process
	parts := strings.Split(uri, "/")
	stack := make([]string, 0, len(parts))

	for _, part := range parts {
		switch part {
		case "", ".":
			// Skip empty parts and current dir markers
			continue
		case "..":
			// Go up one directory if possible
			if len(stack) > 0 {
				stack = stack[:len(stack)-1]
			}
		default:
			stack = append(stack, part)
		}
	}

	// Reconstruct URI
	if len(stack) == 0 {
		return "/"
	}

	normalized := "/" + strings.Join(stack, "/")

	// Remove trailing / unless it's the root
	if normalized != "/" && strings.HasSuffix(normalized, "/") {
		normalized = normalized[:len(normalized)-1]
	}

	return normalized
}

// GetFileExtension extracts the file extension from a URI (without the dot).
func GetFileExtension(uri string) string {
	lastDot := -1
	for i := len(uri) - 1; i >= 0; i-- {
		if uri[i] == '.' {
			lastDot = i
			break
		}
		if uri[i] == '/' {
			break
		}
	}

	if lastDot < 0 {
		return ""
	}

	return uri[lastDot+1:]
}

// GetDirectory extracts the directory part of a URI.
// For example: "/ppt/slides/slide1.xml" -> "/ppt/slides"
// For example: "/ppt/slides/" -> "/ppt/slides"
func GetDirectory(uri string) string {
	// Normalize first to ensure consistent handling
	normalized := NormalizeURI(uri)

	// Now get the directory (all but the last component)
	lastSlash := strings.LastIndex(normalized, "/")

	if lastSlash < 0 {
		return "/"
	}

	if lastSlash == 0 {
		return "/"
	}

	// Check if the part after the last slash contains a dot (likely a file)
	// If not, it's probably a directory
	afterSlash := normalized[lastSlash+1:]
	if strings.Contains(afterSlash, ".") && len(afterSlash) > 0 {
		// It's a file, return everything up to (but not including) the last slash
		return normalized[:lastSlash]
	}

	// It's a directory, return as-is
	return normalized
}

// GetFileName extracts just the filename from a URI.
// For example: "/ppt/slides/slide1.xml" -> "slide1.xml"
func GetFileName(uri string) string {
	lastSlash := -1
	for i := len(uri) - 1; i >= 0; i-- {
		if uri[i] == '/' {
			lastSlash = i
			break
		}
	}

	if lastSlash < 0 {
		return uri
	}

	return uri[lastSlash+1:]
}

// JoinPaths joins two paths, handling relative references like ".." and ".".
// For example: JoinPaths("/ppt/slides", "../slideLayouts/slideLayout1.xml") -> "/ppt/slideLayouts/slideLayout1.xml"
func JoinPaths(basePath, relativePath string) string {
	// Normalize base path
	basePath = NormalizeURI(basePath)
	relativePath = strings.ReplaceAll(relativePath, "\\", "/")

	// If base is a file (has an extension after the last /), get its directory
	lastSlash := strings.LastIndex(basePath, "/")
	lastDot := strings.LastIndex(basePath, ".")
	if lastDot > lastSlash && lastDot > 0 {
		basePath = GetDirectory(basePath)
	}

	// Start with base path parts (excluding empty strings)
	var parts []string
	if basePath == "/" {
		parts = []string{}
	} else {
		for _, p := range strings.Split(basePath, "/") {
			if p != "" {
				parts = append(parts, p)
			}
		}
	}

	// Apply relative path components
	for _, segment := range strings.Split(relativePath, "/") {
		if segment == "" || segment == "." {
			continue
		} else if segment == ".." {
			if len(parts) > 0 {
				parts = parts[:len(parts)-1]
			}
		} else {
			parts = append(parts, segment)
		}
	}

	// Reconstruct the path
	if len(parts) == 0 {
		return "/"
	}
	return "/" + strings.Join(parts, "/")
}

// IsRelsFile returns true if the URI represents a .rels file.
func IsRelsFile(uri string) bool {
	return strings.HasSuffix(uri, ".rels")
}
