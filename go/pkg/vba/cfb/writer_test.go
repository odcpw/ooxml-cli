package cfb

import (
	"bytes"
	"encoding/binary"
	"strings"
	"testing"
)

func TestBuildRegularSectorFileUsesMiniStreamForSmallStreams(t *testing.T) {
	streams := map[string][]byte{
		"PROJECT":             []byte("ID={00000000-0000-0000-0000-000000000000}\r\n"),
		"VBA/dir":             []byte(strings.Repeat("d", 700)),
		"VBA/SmallModule":     []byte(strings.Repeat("s", 300)),
		"VBA/LargeModule":     []byte(strings.Repeat("l", writerMiniStreamCutoff+200)),
		"VBA/_VBA_PROJECT":    []byte(strings.Repeat("p", writerMiniStreamCutoff+1)),
		"VBA/Nested/SmallOne": []byte("nested"),
	}

	data, err := BuildRegularSectorFile(streams)
	if err != nil {
		t.Fatalf("BuildRegularSectorFile failed: %v", err)
	}
	if firstMini := binary.LittleEndian.Uint32(data[60:64]); firstMini == sectorEnd {
		t.Fatalf("expected first mini FAT sector to be set")
	}
	if count := binary.LittleEndian.Uint32(data[64:68]); count == 0 {
		t.Fatalf("expected mini FAT sector count to be non-zero")
	}

	file, err := Open(data)
	if err != nil {
		t.Fatalf("Open(rewritten) failed: %v", err)
	}
	if len(file.miniFAT) == 0 || len(file.miniStream) == 0 {
		t.Fatalf("expected mini FAT and mini stream to be populated")
	}

	for path, want := range streams {
		got, err := file.Stream(path)
		if err != nil {
			t.Fatalf("Stream(%s) failed: %v", path, err)
		}
		if !bytes.Equal(got, want) {
			t.Fatalf("Stream(%s) mismatch: got %d bytes, want %d", path, len(got), len(want))
		}
	}

	for _, path := range []string{"PROJECT", "VBA/dir", "VBA/SmallModule", "VBA/Nested/SmallOne"} {
		entry := file.streams[normalizePath(path)]
		if entry == nil {
			t.Fatalf("missing directory entry for %s", path)
		}
		if entry.Size >= writerMiniStreamCutoff {
			t.Fatalf("test setup error: %s is not a small stream", path)
		}
		if entry.StartSector == sectorEnd || int(entry.StartSector) >= len(file.miniFAT) {
			t.Fatalf("%s start sector %d should be a mini sector within mini FAT length %d", path, entry.StartSector, len(file.miniFAT))
		}
		if file.miniFAT[entry.StartSector] == sectorFree {
			t.Fatalf("%s mini sector %d is not linked in the mini FAT", path, entry.StartSector)
		}
	}

	for _, path := range []string{"VBA/LargeModule", "VBA/_VBA_PROJECT"} {
		entry := file.streams[normalizePath(path)]
		if entry == nil {
			t.Fatalf("missing directory entry for %s", path)
		}
		if entry.Size < writerMiniStreamCutoff {
			t.Fatalf("test setup error: %s is not a large stream", path)
		}
		if entry.StartSector == sectorEnd || int(entry.StartSector) >= len(file.fat) {
			t.Fatalf("%s start sector %d should be a regular FAT sector within FAT length %d", path, entry.StartSector, len(file.fat))
		}
		if file.fat[entry.StartSector] == sectorFree {
			t.Fatalf("%s regular sector %d is not linked in the FAT", path, entry.StartSector)
		}
	}
}
