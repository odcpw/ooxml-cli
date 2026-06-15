package cfb

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"strings"
	"unicode/utf16"
)

const (
	maxRegularSectorChain = 1 << 20
	maxMiniSectorChain    = 1 << 22

	sectorFree       uint32 = 0xFFFFFFFF
	sectorEnd        uint32 = 0xFFFFFFFE
	sectorFAT        uint32 = 0xFFFFFFFD
	sectorDIFAT      uint32 = 0xFFFFFFFC
	directoryStream         = 2
	directoryStorage        = 1
	directoryRoot           = 5
)

var compoundSignature = []byte{0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1}

// File is a read-only Compound File Binary container.
type File struct {
	data             []byte
	sectorSize       int
	miniSectorSize   int
	miniStreamCutoff uint64
	fat              []uint32
	miniFAT          []uint32
	miniStream       []byte
	entries          []directoryEntry
	streams          map[string]*directoryEntry
}

type directoryEntry struct {
	Name         string
	Path         string
	ObjectType   byte
	Color        byte
	LeftSibling  uint32
	RightSibling uint32
	Child        uint32
	CLSID        [16]byte
	StateBits    uint32
	CreationTime [8]byte
	ModifiedTime [8]byte
	StartSector  uint32
	Size         uint64
}

// Open parses a read-only CFB file.
func Open(data []byte) (*File, error) {
	if len(data) < 512 || !bytes.Equal(data[:8], compoundSignature) {
		return nil, fmt.Errorf("not a Compound File Binary vbaProject.bin")
	}
	if binary.LittleEndian.Uint16(data[28:30]) != 0xFFFE {
		return nil, fmt.Errorf("unsupported CFB byte order")
	}

	sectorShift := binary.LittleEndian.Uint16(data[30:32])
	miniSectorShift := binary.LittleEndian.Uint16(data[32:34])
	if sectorShift < 9 || sectorShift > 12 {
		return nil, fmt.Errorf("unsupported CFB sector size shift %d", sectorShift)
	}
	if miniSectorShift != 6 {
		return nil, fmt.Errorf("unsupported CFB mini sector size shift %d", miniSectorShift)
	}

	file := &File{
		data:             data,
		sectorSize:       1 << sectorShift,
		miniSectorSize:   1 << miniSectorShift,
		miniStreamCutoff: uint64(binary.LittleEndian.Uint32(data[56:60])),
		streams:          map[string]*directoryEntry{},
	}
	if file.miniStreamCutoff == 0 {
		file.miniStreamCutoff = 4096
	}

	numFATSectors := binary.LittleEndian.Uint32(data[44:48])
	firstDirectorySector := binary.LittleEndian.Uint32(data[48:52])
	firstMiniFATSector := binary.LittleEndian.Uint32(data[60:64])
	numMiniFATSectors := binary.LittleEndian.Uint32(data[64:68])
	firstDIFATSector := binary.LittleEndian.Uint32(data[68:72])
	numDIFATSectors := binary.LittleEndian.Uint32(data[72:76])

	fatSectors, err := file.collectFATSectors(numFATSectors, firstDIFATSector, numDIFATSectors)
	if err != nil {
		return nil, err
	}
	if err := file.readFAT(fatSectors); err != nil {
		return nil, err
	}
	if firstMiniFATSector != sectorEnd && numMiniFATSectors > 0 {
		if err := file.readMiniFAT(firstMiniFATSector, numMiniFATSectors); err != nil {
			return nil, err
		}
	}

	directoryData, err := file.readRegularStream(firstDirectorySector, 0)
	if err != nil {
		return nil, fmt.Errorf("failed to read CFB directory stream: %w", err)
	}
	if err := file.parseDirectory(directoryData); err != nil {
		return nil, err
	}
	if err := file.buildPaths(); err != nil {
		return nil, err
	}
	if root := file.rootEntry(); root != nil && root.StartSector != sectorEnd && root.Size > 0 {
		miniStream, err := file.readRegularStream(root.StartSector, root.Size)
		if err != nil {
			return nil, fmt.Errorf("failed to read CFB mini stream: %w", err)
		}
		file.miniStream = miniStream
	}

	return file, nil
}

// Stream returns one stream by slash-separated storage path.
func (f *File) Stream(path string) ([]byte, error) {
	normalized := normalizePath(path)
	entry := f.streams[normalized]
	if entry == nil {
		for candidatePath, candidate := range f.streams {
			if strings.EqualFold(candidatePath, normalized) {
				entry = candidate
				break
			}
		}
	}
	if entry == nil {
		return nil, fmt.Errorf("CFB stream %q not found", path)
	}
	if entry.ObjectType != directoryStream {
		return nil, fmt.Errorf("CFB path %q is not a stream", path)
	}
	if entry.Size == 0 {
		return nil, nil
	}
	if entry.Size < f.miniStreamCutoff && len(f.miniFAT) > 0 && len(f.miniStream) > 0 {
		return f.readMiniStream(entry.StartSector, entry.Size)
	}
	return f.readRegularStream(entry.StartSector, entry.Size)
}

// Streams returns the stream paths in directory traversal order.
func (f *File) Streams() []string {
	paths := make([]string, 0, len(f.streams))
	for _, entry := range f.entries {
		if entry.ObjectType == directoryStream && entry.Path != "" {
			paths = append(paths, entry.Path)
		}
	}
	return paths
}

func (f *File) collectFATSectors(numFATSectors, firstDIFATSector, numDIFATSectors uint32) ([]uint32, error) {
	var sectors []uint32
	for offset := 76; offset < 512 && uint32(len(sectors)) < numFATSectors; offset += 4 {
		sector := binary.LittleEndian.Uint32(f.data[offset : offset+4])
		if sector != sectorFree && sector != sectorEnd {
			sectors = append(sectors, sector)
		}
	}
	current := firstDIFATSector
	for i := uint32(0); i < numDIFATSectors && current != sectorEnd && uint32(len(sectors)) < numFATSectors; i++ {
		sectorData, err := f.sector(current)
		if err != nil {
			return nil, fmt.Errorf("failed to read DIFAT sector %d: %w", current, err)
		}
		entriesPerDIFAT := f.sectorSize/4 - 1
		for j := 0; j < entriesPerDIFAT && uint32(len(sectors)) < numFATSectors; j++ {
			sector := binary.LittleEndian.Uint32(sectorData[j*4 : j*4+4])
			if sector != sectorFree && sector != sectorEnd {
				sectors = append(sectors, sector)
			}
		}
		current = binary.LittleEndian.Uint32(sectorData[f.sectorSize-4:])
	}
	if uint32(len(sectors)) < numFATSectors {
		return nil, fmt.Errorf("CFB DIFAT listed %d FAT sectors, want %d", len(sectors), numFATSectors)
	}
	return sectors, nil
}

func (f *File) readFAT(fatSectors []uint32) error {
	for _, fatSector := range fatSectors {
		sectorData, err := f.sector(fatSector)
		if err != nil {
			return fmt.Errorf("failed to read FAT sector %d: %w", fatSector, err)
		}
		for offset := 0; offset < len(sectorData); offset += 4 {
			f.fat = append(f.fat, binary.LittleEndian.Uint32(sectorData[offset:offset+4]))
		}
	}
	return nil
}

func (f *File) readMiniFAT(firstSector, numSectors uint32) error {
	chain, err := f.regularSectorChain(firstSector, uint64(numSectors)*uint64(f.sectorSize), int(numSectors)+1)
	if err != nil {
		return fmt.Errorf("failed to read mini FAT chain: %w", err)
	}
	for _, sectorData := range chain {
		for offset := 0; offset < len(sectorData); offset += 4 {
			f.miniFAT = append(f.miniFAT, binary.LittleEndian.Uint32(sectorData[offset:offset+4]))
		}
	}
	return nil
}

func (f *File) parseDirectory(data []byte) error {
	if len(data)%128 != 0 {
		return fmt.Errorf("CFB directory stream size %d is not a multiple of 128", len(data))
	}
	for offset := 0; offset+128 <= len(data); offset += 128 {
		raw := data[offset : offset+128]
		nameLen := int(binary.LittleEndian.Uint16(raw[64:66]))
		if nameLen > 64 {
			nameLen = 64
		}
		name := ""
		if nameLen >= 2 {
			name = decodeUTF16Name(raw[:nameLen-2])
		}
		entry := directoryEntry{
			Name:         name,
			ObjectType:   raw[66],
			Color:        raw[67],
			LeftSibling:  binary.LittleEndian.Uint32(raw[68:72]),
			RightSibling: binary.LittleEndian.Uint32(raw[72:76]),
			Child:        binary.LittleEndian.Uint32(raw[76:80]),
			StateBits:    binary.LittleEndian.Uint32(raw[96:100]),
			StartSector:  binary.LittleEndian.Uint32(raw[116:120]),
			Size:         binary.LittleEndian.Uint64(raw[120:128]),
		}
		copy(entry.CLSID[:], raw[80:96])
		copy(entry.CreationTime[:], raw[100:108])
		copy(entry.ModifiedTime[:], raw[108:116])
		if f.sectorSize == 512 {
			entry.Size = uint64(binary.LittleEndian.Uint32(raw[120:124]))
		}
		f.entries = append(f.entries, entry)
	}
	if len(f.entries) == 0 || f.entries[0].ObjectType != directoryRoot {
		return fmt.Errorf("CFB root directory entry not found")
	}
	return nil
}

func (f *File) buildPaths() error {
	root := f.rootEntry()
	if root == nil {
		return fmt.Errorf("CFB root directory entry not found")
	}
	visited := map[uint32]bool{}
	return f.walkTree(root.Child, "", visited)
}

func (f *File) walkTree(index uint32, parent string, visited map[uint32]bool) error {
	if index == sectorFree || index == sectorEnd {
		return nil
	}
	if int(index) >= len(f.entries) {
		return fmt.Errorf("CFB directory index %d out of range", index)
	}
	if visited[index] {
		return nil
	}
	visited[index] = true
	entry := &f.entries[index]
	if err := f.walkTree(entry.LeftSibling, parent, visited); err != nil {
		return err
	}
	if entry.Name != "" {
		path := normalizePath(parent + "/" + entry.Name)
		entry.Path = path
		if entry.ObjectType == directoryStream {
			f.streams[path] = entry
		}
		if entry.ObjectType == directoryStorage {
			if err := f.walkTree(entry.Child, path, visited); err != nil {
				return err
			}
		}
	}
	return f.walkTree(entry.RightSibling, parent, visited)
}

func (f *File) readRegularStream(firstSector uint32, size uint64) ([]byte, error) {
	chunks, err := f.regularSectorChain(firstSector, size, maxRegularSectorChain)
	if err != nil {
		return nil, err
	}
	data := bytes.Join(chunks, nil)
	if size > 0 && uint64(len(data)) > size {
		data = data[:size]
	}
	return data, nil
}

func (f *File) regularSectorChain(firstSector uint32, size uint64, maxSectors int) ([][]byte, error) {
	if firstSector == sectorEnd || firstSector == sectorFree {
		if size == 0 {
			return nil, nil
		}
		return nil, fmt.Errorf("stream has no starting sector")
	}
	var chunks [][]byte
	current := firstSector
	for current != sectorEnd {
		if current == sectorFree || current == sectorFAT || current == sectorDIFAT {
			return nil, fmt.Errorf("invalid sector marker 0x%08x in stream chain", current)
		}
		if int(current) >= len(f.fat) {
			return nil, fmt.Errorf("sector %d outside FAT", current)
		}
		sectorData, err := f.sector(current)
		if err != nil {
			return nil, err
		}
		chunks = append(chunks, sectorData)
		if len(chunks) > maxSectors {
			return nil, fmt.Errorf("sector chain exceeded safety limit")
		}
		if size > 0 && uint64(len(chunks)*f.sectorSize) >= size {
			break
		}
		current = f.fat[current]
	}
	return chunks, nil
}

func (f *File) readMiniStream(firstMiniSector uint32, size uint64) ([]byte, error) {
	if firstMiniSector == sectorEnd || firstMiniSector == sectorFree {
		if size == 0 {
			return nil, nil
		}
		return nil, fmt.Errorf("mini stream has no starting sector")
	}
	var out []byte
	current := firstMiniSector
	for current != sectorEnd {
		if int(current) >= len(f.miniFAT) {
			return nil, fmt.Errorf("mini sector %d outside mini FAT", current)
		}
		start := int(current) * f.miniSectorSize
		end := start + f.miniSectorSize
		if end > len(f.miniStream) {
			return nil, fmt.Errorf("mini sector %d outside mini stream", current)
		}
		out = append(out, f.miniStream[start:end]...)
		if len(out) > maxMiniSectorChain*f.miniSectorSize {
			return nil, fmt.Errorf("mini sector chain exceeded safety limit")
		}
		if uint64(len(out)) >= size {
			break
		}
		current = f.miniFAT[current]
	}
	if uint64(len(out)) > size {
		out = out[:size]
	}
	return out, nil
}

func (f *File) sector(index uint32) ([]byte, error) {
	start := 512 + int(index)*f.sectorSize
	end := start + f.sectorSize
	if start < 512 || end > len(f.data) {
		return nil, fmt.Errorf("sector %d outside file", index)
	}
	return f.data[start:end], nil
}

func (f *File) rootEntry() *directoryEntry {
	if len(f.entries) == 0 {
		return nil
	}
	return &f.entries[0]
}

func normalizePath(path string) string {
	path = strings.TrimSpace(strings.ReplaceAll(path, "\\", "/"))
	path = strings.Trim(path, "/")
	parts := strings.FieldsFunc(path, func(r rune) bool { return r == '/' })
	return strings.Join(parts, "/")
}

func decodeUTF16Name(data []byte) string {
	units := make([]uint16, 0, len(data)/2)
	for i := 0; i+1 < len(data); i += 2 {
		value := binary.LittleEndian.Uint16(data[i : i+2])
		if value == 0 {
			break
		}
		units = append(units, value)
	}
	return string(utf16.Decode(units))
}
