package vba

import (
	"encoding/binary"

	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/vba/cfb"
)

// fuzzDirRecord encodes a single MS-OVBA dir-stream record (id + uint32 size +
// payload). Mirrors the on-disk format that parseDirStream consumes.
func fuzzDirRecord(id uint16, payload []byte) []byte {
	out := make([]byte, 6+len(payload))
	binary.LittleEndian.PutUint16(out[:2], id)
	binary.LittleEndian.PutUint32(out[2:6], uint32(len(payload)))
	copy(out[6:], payload)
	return out
}

func fuzzUTF16LE(s string) []byte {
	out := make([]byte, 0, len(s)*2)
	for _, r := range s {
		out = binary.LittleEndian.AppendUint16(out, uint16(r))
	}
	return out
}

// fuzzDirStream builds a minimal but valid VBA dir stream describing the given
// modules. Each module gets a name, stream name, source offset, and type marker.
func fuzzDirStream(modules []fuzzModule, offsets []uint32) []byte {
	var out []byte
	// PROJECTCODEPAGE (id 0x0003, size 2) = 1252
	out = append(out, fuzzDirRecord(0x0003, []byte{0xE4, 0x04})...)
	// PROJECTMODULES (id 0x000F, size 2) = module count
	count := make([]byte, 2)
	binary.LittleEndian.PutUint16(count, uint16(len(modules)))
	out = append(out, fuzzDirRecord(0x000F, count)...)
	for idx, m := range modules {
		var off uint32
		if idx < len(offsets) {
			off = offsets[idx]
		}
		out = append(out, fuzzDirRecord(0x0019, []byte(m.Name))...)                       // MODULENAME
		out = append(out, fuzzDirRecord(0x0047, fuzzUTF16LE(m.Name))...)                  // MODULENAMEUNICODE
		out = append(out, fuzzDirRecord(0x001A, []byte(m.StreamName))...)                 // MODULESTREAMNAME
		out = append(out, fuzzDirRecord(0x0032, fuzzUTF16LE(m.StreamName))...)            // MODULESTREAMNAMEUNICODE
		offBytes := make([]byte, 4)
		binary.LittleEndian.PutUint32(offBytes, off)
		out = append(out, fuzzDirRecord(0x0031, offBytes)...) // MODULEOFFSET
		if m.Class {
			out = append(out, fuzzDirRecord(0x0022, nil)...) // MODULETYPE class
		} else {
			out = append(out, fuzzDirRecord(0x0021, nil)...) // MODULETYPE standard
		}
		out = append(out, fuzzDirRecord(0x002B, nil)...) // MODULE terminator
	}
	out = append(out, fuzzDirRecord(0x0010, nil)...) // PROJECTMODULES terminator-ish
	return out
}

type fuzzModule struct {
	Name       string
	StreamName string
	Class      bool
	Source     string
}

// fuzzSyntheticVBAProjectBin builds a real, parseable vbaProject.bin CFB payload
// from the given modules and extra streams, using only exported package helpers.
func fuzzSyntheticVBAProjectBin(modules []fuzzModule, extra map[string][]byte) ([]byte, error) {
	offsets := make([]uint32, 0, len(modules))
	streams := map[string][]byte{
		"VBA/_VBA_PROJECT": {0xCC, 0x61},
	}
	for path, data := range extra {
		streams[path] = append([]byte(nil), data...)
	}
	for _, m := range modules {
		offsets = append(offsets, 0)
		streams["VBA/"+m.StreamName] = CompressContainerLiterals([]byte(m.Source))
	}
	streams["VBA/dir"] = CompressContainerLiterals(fuzzDirStream(modules, offsets))
	return cfb.BuildRegularSectorFile(streams)
}

// fuzzSeeds returns a handful of valid and adversarial vbaProject.bin payloads.
func fuzzSeeds() [][]byte {
	var seeds [][]byte

	// Seed 1: two modules, one standard, one class.
	if bin, err := fuzzSyntheticVBAProjectBin([]fuzzModule{
		{Name: "Module1", StreamName: "Module1", Source: "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n"},
		{Name: "Class1", StreamName: "Class1", Class: true, Source: "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n"},
	}, nil); err == nil {
		seeds = append(seeds, bin)
	}

	// Seed 2: includes a PROJECT text stream and a __SRP cache stream.
	if bin, err := fuzzSyntheticVBAProjectBin([]fuzzModule{
		{Name: "Module1", StreamName: "Module1", Source: "Attribute VB_Name = \"Module1\"\r\nSub A()\r\nEnd Sub\r\n"},
	}, map[string][]byte{
		"PROJECT":     []byte("ID=\"{00000000-0000-0000-0000-000000000000}\"\r\nModule=Module1\r\n[Workspace]\r\nModule1=0, 0, 0, 0, C\r\n"),
		"VBA/__SRP_0": []byte("compiled cache"),
		"PROJECTwm":   append(append([]byte("Module1\x00"), fuzzUTF16LE("Module1")...), 0, 0, 0, 0),
	}); err == nil {
		seeds = append(seeds, bin)
	}

	// Seed 3: a module whose declared source offset is past the stream end
	// (exercises the bounds-check warning path).
	if bin, err := fuzzSyntheticVBAProjectBin([]fuzzModule{
		{Name: "Big", StreamName: "Big", Source: "x"},
	}, nil); err == nil {
		seeds = append(seeds, bin)
	}

	return seeds
}

// FuzzVBA drives the full vbaProject.bin ingest pipeline: CFB compound-file
// parsing, MS-OVBA stream decompression, VBA dir-stream record parsing, module
// source extraction, and PROJECT metadata parsing. A returned error is correct
// behavior for malformed input; only a panic or hang is a bug.
func FuzzVBA(f *testing.F) {
	for _, seed := range fuzzSeeds() {
		f.Add(seed, "pptx")
		f.Add(seed, "xlsx")
	}
	// Minimal adversarial seeds.
	f.Add([]byte{}, "pptx")
	f.Add([]byte("not a cfb file"), "xlsx")
	// CFB signature with nothing else.
	f.Add([]byte{0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1}, "pptx")

	f.Fuzz(func(t *testing.T, data []byte, family string) {
		// Top-level family-annotated entry point. Ignore the error: malformed
		// input is expected to error, never panic.
		if project, err := ParseSourceProjectForFamily(data, family); err == nil {
			// Touch returned fields defensively without asserting correctness,
			// to ensure derived data does not panic on summarization.
			_ = SummarizeSourceProject(project)
			for _, m := range project.Modules {
				_ = ModuleOutputName(m)
			}
		}
	})
}

// FuzzDecompressContainer drives the standalone MS-OVBA compressed-container
// decompressor (chunk headers, copy tokens, literal runs). This hand-rolled
// bit-twiddling decompressor is a high-value target on its own.
func FuzzDecompressContainer(f *testing.F) {
	f.Add(CompressContainerLiterals([]byte("hello world")))
	f.Add(CompressContainerLiterals([]byte("")))
	f.Add([]byte{0x01})
	f.Add([]byte{0x01, 0x00, 0x30})
	f.Add([]byte{0x00})

	f.Fuzz(func(t *testing.T, data []byte) {
		_, _ = DecompressContainer(data)
	})
}
