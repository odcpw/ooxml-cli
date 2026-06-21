package cfb

import (
	"testing"
)

// FuzzCFBOpen drives the read-only Compound File Binary parser directly:
// header validation, FAT/DIFAT collection, mini-FAT, directory tree walking,
// and sector-chain following. This hand-rolled binary container parser is a
// high-value fuzz target. A returned error is correct behavior for malformed
// input; only a panic or hang is a bug.
func FuzzCFBOpen(f *testing.F) {
	// A valid CFB built from the writer makes the best seed.
	if data, err := BuildRegularSectorFile(map[string][]byte{
		"PROJECT":          []byte("ID={00000000-0000-0000-0000-000000000000}\r\n"),
		"VBA/dir":          []byte("dir stream contents that are reasonably long"),
		"VBA/Module1":      []byte("module one source bytes"),
		"VBA/_VBA_PROJECT": []byte{0xCC, 0x61, 0x00, 0x00},
	}); err == nil {
		f.Add(data)
	}
	if data, err := BuildRegularSectorFile(map[string][]byte{
		"VBA/Big": make([]byte, 9000), // forces a multi-sector regular stream
	}); err == nil {
		f.Add(data)
	}

	// Adversarial seeds.
	f.Add([]byte{})
	f.Add(make([]byte, 512)) // zeroed 512 bytes, no signature
	sig := make([]byte, 512)
	copy(sig, compoundSignature)
	f.Add(sig) // valid signature, garbage everywhere else

	f.Fuzz(func(t *testing.T, data []byte) {
		file, err := Open(data)
		if err != nil {
			return
		}
		// Walk every advertised stream to exercise the chain followers. Errors
		// are fine; panics/hangs are bugs.
		for _, path := range file.Streams() {
			_, _ = file.Stream(path)
		}
	})
}
