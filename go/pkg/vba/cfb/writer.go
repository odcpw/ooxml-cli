package cfb

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"sort"
	"strings"
	"unicode/utf16"
)

const (
	writerSectorSize       = 512
	writerSectorShift      = 9
	writerMiniSectorShift  = 6
	writerMiniSectorSize   = 64
	writerMiniStreamCutoff = 4096
	writerMaxDIFATEntries  = 109
)

// RewriteStreams rebuilds a Compound File Binary container using regular
// sectors for every stream. It preserves stream paths and replaces any paths
// present in replacements.
func RewriteStreams(data []byte, replacements map[string][]byte) ([]byte, error) {
	return RewriteStreamsWithDeletes(data, replacements, nil)
}

// RewriteStreamsWithDeletes rebuilds a CFB container, applying stream
// replacements and removing exact stream paths listed in deletes.
func RewriteStreamsWithDeletes(data []byte, replacements map[string][]byte, deletes []string) ([]byte, error) {
	return RewriteStreamsWithAddsAndDeletes(data, replacements, nil, deletes)
}

// RewriteStreamsWithAddsAndDeletes rebuilds a CFB container, applying stream
// replacements, adding new streams, and removing exact stream paths listed in
// deletes.
func RewriteStreamsWithAddsAndDeletes(data []byte, replacements map[string][]byte, additions map[string][]byte, deletes []string) ([]byte, error) {
	file, err := Open(data)
	if err != nil {
		return nil, err
	}

	deleteSet := map[string]bool{}
	for _, path := range deletes {
		deleteSet[normalizePath(path)] = true
	}

	streams := map[string][]byte{}
	for _, path := range file.Streams() {
		if deleteSet[normalizePath(path)] {
			continue
		}
		streamData, err := file.Stream(path)
		if err != nil {
			return nil, err
		}
		streams[normalizePath(path)] = streamData
	}
	for path, streamData := range replacements {
		normalized := normalizePath(path)
		if _, ok := streams[normalized]; !ok {
			return nil, fmt.Errorf("CFB stream %q not found", path)
		}
		streams[normalized] = append([]byte(nil), streamData...)
	}
	for path, streamData := range additions {
		normalized := normalizePath(path)
		if _, ok := streams[normalized]; ok {
			return nil, fmt.Errorf("CFB stream %q already exists", path)
		}
		streams[normalized] = append([]byte(nil), streamData...)
	}
	return buildRegularSectorFile(streams, file)
}

// BuildRegularSectorFile creates a deterministic CFB file. Streams smaller than
// the standard mini-stream cutoff are stored through the mini FAT; larger
// streams use regular FAT sectors. It is intentionally small-scope: no DIFAT
// extension sectors.
func BuildRegularSectorFile(streams map[string][]byte) ([]byte, error) {
	return buildRegularSectorFile(streams, nil)
}

func buildRegularSectorFile(streams map[string][]byte, source *File) ([]byte, error) {
	if len(streams) == 0 {
		return nil, fmt.Errorf("cannot build CFB file with no streams")
	}

	entries, children, streamOrder, preserveTree, err := buildDirectoryEntries(streams, source)
	if err != nil {
		return nil, err
	}

	regularStreamSectors := map[int][]uint32{}
	var regularSectors [][]byte
	miniChains := map[int][]uint32{}
	var miniFAT []uint32
	var miniStream []byte
	for _, entryIndex := range streamOrder {
		entry := &entries[entryIndex]
		data := streams[entry.Path]
		entry.Size = uint64(len(data))
		if len(data) == 0 {
			entry.StartSector = sectorEnd
			continue
		}
		if len(data) < writerMiniStreamCutoff {
			padded := append([]byte(nil), data...)
			for len(padded)%writerMiniSectorSize != 0 {
				padded = append(padded, 0)
			}
			start := uint32(len(miniFAT))
			entry.StartSector = start
			for len(padded) > 0 {
				miniStream = append(miniStream, padded[:writerMiniSectorSize]...)
				miniChains[entryIndex] = append(miniChains[entryIndex], uint32(len(miniFAT)))
				miniFAT = append(miniFAT, sectorFree)
				padded = padded[writerMiniSectorSize:]
			}
			continue
		}
		padded := append([]byte(nil), data...)
		for len(padded)%writerSectorSize != 0 {
			padded = append(padded, 0)
		}
		start := uint32(len(regularSectors))
		for len(padded) > 0 {
			regularSectors = append(regularSectors, append([]byte(nil), padded[:writerSectorSize]...))
			regularStreamSectors[entryIndex] = append(regularStreamSectors[entryIndex], start+uint32(len(regularStreamSectors[entryIndex])))
			padded = padded[writerSectorSize:]
		}
	}
	for _, chain := range miniChains {
		linkChain(miniFAT, chain)
	}

	var miniStreamSectors []uint32
	if len(miniStream) > 0 {
		entries[0].Size = uint64(len(miniStream))
		padded := append([]byte(nil), miniStream...)
		for len(padded)%writerSectorSize != 0 {
			padded = append(padded, 0)
		}
		start := uint32(len(regularSectors))
		for len(padded) > 0 {
			regularSectors = append(regularSectors, append([]byte(nil), padded[:writerSectorSize]...))
			miniStreamSectors = append(miniStreamSectors, start+uint32(len(miniStreamSectors)))
			padded = padded[writerSectorSize:]
		}
	} else {
		entries[0].StartSector = sectorEnd
		entries[0].Size = 0
	}

	if !preserveTree {
		for parentIndex, childIndexes := range children {
			if len(childIndexes) > 0 {
				entries[parentIndex].Child = uint32(assignDirectorySiblingTree(entries, childIndexes))
			}
		}
	}

	directorySectorCount := sectorsNeeded(len(entries)*128, writerSectorSize)
	miniFATSectorCount := sectorsNeeded(len(miniFAT)*4, writerSectorSize)

	dataSectorCount := len(regularSectors) + miniFATSectorCount + directorySectorCount
	fatSectorCount := 1
	for {
		next := sectorsNeeded(dataSectorCount+fatSectorCount, writerSectorSize/4)
		if next == fatSectorCount {
			break
		}
		fatSectorCount = next
	}
	if fatSectorCount > writerMaxDIFATEntries {
		return nil, fmt.Errorf("CFB file needs %d FAT sectors; writer supports at most %d", fatSectorCount, writerMaxDIFATEntries)
	}

	sectorBase := uint32(fatSectorCount)
	for entryIndex, sectors := range regularStreamSectors {
		entries[entryIndex].StartSector = sectorBase + sectors[0]
	}
	if len(miniStreamSectors) > 0 {
		entries[0].StartSector = sectorBase + miniStreamSectors[0]
	}
	miniFATStart := uint32(sectorEnd)
	if miniFATSectorCount > 0 {
		miniFATStart = sectorBase + uint32(len(regularSectors))
	}
	directoryStart := sectorBase + uint32(len(regularSectors)+miniFATSectorCount)
	for i := range regularStreamSectors {
		for j := range regularStreamSectors[i] {
			regularStreamSectors[i][j] += sectorBase
		}
	}
	for i := range miniStreamSectors {
		miniStreamSectors[i] += sectorBase
	}

	directoryData := serializeDirectory(entries)
	var directorySectors [][]byte
	for len(directoryData) > 0 {
		directorySectors = append(directorySectors, append([]byte(nil), directoryData[:writerSectorSize]...))
		directoryData = directoryData[writerSectorSize:]
	}
	var miniFATSectors [][]byte
	if miniFATSectorCount > 0 {
		miniFATData := make([]byte, miniFATSectorCount*writerSectorSize)
		for i, value := range miniFAT {
			binary.LittleEndian.PutUint32(miniFATData[i*4:i*4+4], value)
		}
		for i := len(miniFAT); i < len(miniFATData)/4; i++ {
			binary.LittleEndian.PutUint32(miniFATData[i*4:i*4+4], sectorFree)
		}
		for len(miniFATData) > 0 {
			miniFATSectors = append(miniFATSectors, append([]byte(nil), miniFATData[:writerSectorSize]...))
			miniFATData = miniFATData[writerSectorSize:]
		}
	}

	totalSectors := fatSectorCount + dataSectorCount
	fat := make([]uint32, totalSectors)
	for i := range fat {
		fat[i] = sectorFree
	}
	for i := 0; i < fatSectorCount; i++ {
		fat[i] = sectorFAT
	}
	for _, sectors := range regularStreamSectors {
		linkChain(fat, sectors)
	}
	linkChain(fat, miniStreamSectors)
	if miniFATSectorCount > 0 {
		miniFATChain := make([]uint32, miniFATSectorCount)
		for i := range miniFATChain {
			miniFATChain[i] = miniFATStart + uint32(i)
		}
		linkChain(fat, miniFATChain)
	}
	directoryChain := make([]uint32, len(directorySectors))
	for i := range directoryChain {
		directoryChain[i] = directoryStart + uint32(i)
	}
	linkChain(fat, directoryChain)

	header := buildHeader(uint32(fatSectorCount), directoryStart, miniFATStart, uint32(miniFATSectorCount))
	var out bytes.Buffer
	out.Write(header)
	for fatIndex := 0; fatIndex < fatSectorCount; fatIndex++ {
		sector := make([]byte, writerSectorSize)
		start := fatIndex * writerSectorSize / 4
		for i := 0; i < writerSectorSize/4; i++ {
			value := uint32(sectorFree)
			if start+i < len(fat) {
				value = fat[start+i]
			}
			binary.LittleEndian.PutUint32(sector[i*4:i*4+4], value)
		}
		out.Write(sector)
	}
	for _, sector := range regularSectors {
		out.Write(sector)
	}
	for _, sector := range miniFATSectors {
		out.Write(sector)
	}
	for _, sector := range directorySectors {
		out.Write(sector)
	}
	return out.Bytes(), nil
}

type writeDirectoryEntry struct {
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
	Parent       string
}

func buildDirectoryEntries(streams map[string][]byte, source *File) ([]writeDirectoryEntry, map[int][]int, []int, bool, error) {
	if source != nil {
		return buildDirectoryEntriesFromSource(streams, source)
	}
	return buildDirectoryEntriesSorted(streams)
}

func buildDirectoryEntriesSorted(streams map[string][]byte) ([]writeDirectoryEntry, map[int][]int, []int, bool, error) {
	const rootPath = ""
	entries := []writeDirectoryEntry{{
		Name:         "Root Entry",
		ObjectType:   directoryRoot,
		Color:        1,
		LeftSibling:  sectorFree,
		RightSibling: sectorFree,
		Child:        sectorFree,
		StartSector:  sectorEnd,
		Path:         rootPath,
	}}
	pathToIndex := map[string]int{rootPath: 0}
	parentChildren := map[string][]int{}

	paths := make([]string, 0, len(streams))
	for path := range streams {
		normalized := normalizePath(path)
		if normalized == "" {
			return nil, nil, nil, false, fmt.Errorf("CFB stream path cannot be empty")
		}
		paths = append(paths, normalized)
	}
	sort.Slice(paths, func(i, j int) bool {
		return directoryPathLess(paths[i], paths[j])
	})

	for _, path := range paths {
		parts := strings.Split(path, "/")
		parent := rootPath
		for i := 0; i < len(parts)-1; i++ {
			storagePath := strings.Join(parts[:i+1], "/")
			if _, ok := pathToIndex[storagePath]; ok {
				parent = storagePath
				continue
			}
			if err := validateDirectoryName(parts[i]); err != nil {
				return nil, nil, nil, false, err
			}
			index := len(entries)
			entries = append(entries, writeDirectoryEntry{
				Name:         parts[i],
				Path:         storagePath,
				ObjectType:   directoryStorage,
				Color:        1,
				LeftSibling:  sectorFree,
				RightSibling: sectorFree,
				Child:        sectorFree,
				StartSector:  sectorEnd,
				Parent:       parent,
			})
			pathToIndex[storagePath] = index
			parentChildren[parent] = append(parentChildren[parent], index)
			parent = storagePath
		}
		if err := validateDirectoryName(parts[len(parts)-1]); err != nil {
			return nil, nil, nil, false, err
		}
		index := len(entries)
		entries = append(entries, writeDirectoryEntry{
			Name:         parts[len(parts)-1],
			Path:         path,
			ObjectType:   directoryStream,
			Color:        1,
			LeftSibling:  sectorFree,
			RightSibling: sectorFree,
			Child:        sectorFree,
			StartSector:  sectorEnd,
			Parent:       parent,
		})
		pathToIndex[path] = index
		parentChildren[parent] = append(parentChildren[parent], index)
	}

	childrenByIndex := map[int][]int{}
	for parentPath, childIndexes := range parentChildren {
		sort.SliceStable(childIndexes, func(i, j int) bool {
			return strings.ToLower(entries[childIndexes[i]].Name) < strings.ToLower(entries[childIndexes[j]].Name)
		})
		parentIndex := pathToIndex[parentPath]
		childrenByIndex[parentIndex] = childIndexes
	}

	streamOrder := make([]int, 0, len(streams))
	for idx, entry := range entries {
		if entry.ObjectType == directoryStream {
			streamOrder = append(streamOrder, idx)
		}
	}
	sort.SliceStable(streamOrder, func(i, j int) bool {
		return directoryPathLess(entries[streamOrder[i]].Path, entries[streamOrder[j]].Path)
	})
	return entries, childrenByIndex, streamOrder, false, nil
}

func buildDirectoryEntriesFromSource(streams map[string][]byte, source *File) ([]writeDirectoryEntry, map[int][]int, []int, bool, error) {
	const rootPath = ""
	neededStorages := map[string]bool{}
	for path := range streams {
		parts := strings.Split(normalizePath(path), "/")
		for i := 0; i < len(parts)-1; i++ {
			neededStorages[strings.Join(parts[:i+1], "/")] = true
		}
	}

	entries := make([]writeDirectoryEntry, len(source.entries))
	pathToIndex := map[string]int{rootPath: 0}
	parentChildren := map[string][]int{}
	sourceStreams := map[string]bool{}
	includedStreamCount := 0

	for i, sourceEntry := range source.entries {
		entry := writeDirectoryEntry{
			Name:         sourceEntry.Name,
			Path:         sourceEntry.Path,
			ObjectType:   sourceEntry.ObjectType,
			Color:        sourceEntry.Color,
			LeftSibling:  sourceEntry.LeftSibling,
			RightSibling: sourceEntry.RightSibling,
			Child:        sourceEntry.Child,
			CLSID:        sourceEntry.CLSID,
			StateBits:    sourceEntry.StateBits,
			CreationTime: sourceEntry.CreationTime,
			ModifiedTime: sourceEntry.ModifiedTime,
			StartSector:  sectorEnd,
			Parent:       parentPath(sourceEntry.Path),
		}
		include := false
		switch sourceEntry.ObjectType {
		case directoryRoot:
			entry.Path = rootPath
			entry.Parent = ""
			include = true
		case directoryStorage:
			include = neededStorages[sourceEntry.Path]
		case directoryStream:
			sourceStreams[sourceEntry.Path] = true
			_, include = streams[sourceEntry.Path]
			if include {
				includedStreamCount++
			}
		}
		if !include {
			entries[i] = writeDirectoryEntry{
				LeftSibling:  sectorFree,
				RightSibling: sectorFree,
				Child:        sectorFree,
				StartSector:  sectorEnd,
			}
			continue
		}
		if entry.Color != 0 && entry.Color != 1 {
			entry.Color = 1
		}
		entries[i] = entry
		pathToIndex[entry.Path] = i
		if entry.ObjectType != directoryRoot {
			parentChildren[entry.Parent] = append(parentChildren[entry.Parent], i)
		}
	}

	var addedPaths []string
	for path := range streams {
		if !sourceStreams[path] {
			addedPaths = append(addedPaths, path)
		}
	}
	sort.Slice(addedPaths, func(i, j int) bool {
		return directoryPathLess(addedPaths[i], addedPaths[j])
	})
	for _, path := range addedPaths {
		parts := strings.Split(path, "/")
		parent := rootPath
		for i := 0; i < len(parts)-1; i++ {
			storagePath := strings.Join(parts[:i+1], "/")
			if _, ok := pathToIndex[storagePath]; ok {
				parent = storagePath
				continue
			}
			if err := validateDirectoryName(parts[i]); err != nil {
				return nil, nil, nil, false, err
			}
			index := len(entries)
			entries = append(entries, writeDirectoryEntry{
				Name:         parts[i],
				Path:         storagePath,
				ObjectType:   directoryStorage,
				Color:        1,
				LeftSibling:  sectorFree,
				RightSibling: sectorFree,
				Child:        sectorFree,
				StartSector:  sectorEnd,
				Parent:       parent,
			})
			pathToIndex[storagePath] = index
			parentChildren[parent] = append(parentChildren[parent], index)
			parent = storagePath
		}
		if err := validateDirectoryName(parts[len(parts)-1]); err != nil {
			return nil, nil, nil, false, err
		}
		index := len(entries)
		entries = append(entries, writeDirectoryEntry{
			Name:         parts[len(parts)-1],
			Path:         path,
			ObjectType:   directoryStream,
			Color:        1,
			LeftSibling:  sectorFree,
			RightSibling: sectorFree,
			Child:        sectorFree,
			StartSector:  sectorEnd,
			Parent:       parent,
		})
		pathToIndex[path] = index
		parentChildren[parent] = append(parentChildren[parent], index)
	}

	childrenByIndex := map[int][]int{}
	for parentPath, childIndexes := range parentChildren {
		sort.SliceStable(childIndexes, func(i, j int) bool {
			return directoryNameLess(entries[childIndexes[i]].Name, entries[childIndexes[j]].Name)
		})
		parentIndex, ok := pathToIndex[parentPath]
		if !ok {
			return nil, nil, nil, false, fmt.Errorf("CFB storage %q missing for child entries", parentPath)
		}
		childrenByIndex[parentIndex] = childIndexes
	}

	streamOrder := make([]int, 0, len(streams))
	for idx, entry := range entries {
		if entry.ObjectType == directoryStream && entry.Path != "" {
			streamOrder = append(streamOrder, idx)
		}
	}
	preserveTree := len(addedPaths) == 0 && includedStreamCount == len(source.streams)
	if preserveTree {
		childrenByIndex = nil
	}
	return entries, childrenByIndex, streamOrder, preserveTree, nil
}

func parentPath(path string) string {
	path = normalizePath(path)
	if path == "" {
		return ""
	}
	idx := strings.LastIndex(path, "/")
	if idx < 0 {
		return ""
	}
	return path[:idx]
}

func directoryNameLess(a, b string) bool {
	aUnits := utf16.Encode([]rune(a))
	bUnits := utf16.Encode([]rune(b))
	if len(aUnits) != len(bUnits) {
		return len(aUnits) < len(bUnits)
	}
	aFold := strings.ToUpper(a)
	bFold := strings.ToUpper(b)
	if aFold != bFold {
		return aFold < bFold
	}
	return a < b
}

func directoryPathLess(a, b string) bool {
	aParts := strings.Split(normalizePath(a), "/")
	bParts := strings.Split(normalizePath(b), "/")
	for i := 0; i < len(aParts) && i < len(bParts); i++ {
		if strings.EqualFold(aParts[i], bParts[i]) {
			continue
		}
		if directoryNameLess(aParts[i], bParts[i]) {
			return true
		}
		return false
	}
	return len(aParts) < len(bParts)
}

func assignDirectorySiblingTree(entries []writeDirectoryEntry, childIndexes []int) int {
	if len(childIndexes) == 0 {
		return int(sectorFree)
	}
	mid := len(childIndexes) / 2
	root := childIndexes[mid]
	if left := assignDirectorySiblingTree(entries, childIndexes[:mid]); left != int(sectorFree) {
		entries[root].LeftSibling = uint32(left)
	}
	if right := assignDirectorySiblingTree(entries, childIndexes[mid+1:]); right != int(sectorFree) {
		entries[root].RightSibling = uint32(right)
	}
	return root
}

func validateDirectoryName(name string) error {
	if strings.TrimSpace(name) == "" {
		return fmt.Errorf("CFB directory name cannot be empty")
	}
	if len(utf16.Encode([]rune(name))) > 31 {
		return fmt.Errorf("CFB directory name %q is longer than 31 UTF-16 code units", name)
	}
	return nil
}

func serializeDirectory(entries []writeDirectoryEntry) []byte {
	size := sectorsNeeded(len(entries)*128, writerSectorSize) * writerSectorSize
	out := make([]byte, 0, size)
	for _, entry := range entries {
		out = append(out, serializeDirectoryEntry(entry)...)
	}
	for len(out)%writerSectorSize != 0 {
		out = append(out, 0)
	}
	return out
}

func serializeDirectoryEntry(entry writeDirectoryEntry) []byte {
	out := make([]byte, 128)
	if entry.ObjectType == 0 && entry.Name == "" {
		return out
	}
	nameBytes := utf16NameBytes(entry.Name)
	copy(out[:64], nameBytes)
	binary.LittleEndian.PutUint16(out[64:66], uint16(len(nameBytes)))
	out[66] = entry.ObjectType
	out[67] = entry.Color
	binary.LittleEndian.PutUint32(out[68:72], entry.LeftSibling)
	binary.LittleEndian.PutUint32(out[72:76], entry.RightSibling)
	binary.LittleEndian.PutUint32(out[76:80], entry.Child)
	copy(out[80:96], entry.CLSID[:])
	binary.LittleEndian.PutUint32(out[96:100], entry.StateBits)
	copy(out[100:108], entry.CreationTime[:])
	copy(out[108:116], entry.ModifiedTime[:])
	binary.LittleEndian.PutUint32(out[116:120], entry.StartSector)
	binary.LittleEndian.PutUint64(out[120:128], entry.Size)
	return out
}

func utf16NameBytes(name string) []byte {
	units := utf16.Encode([]rune(name + "\x00"))
	out := make([]byte, len(units)*2)
	for i, unit := range units {
		binary.LittleEndian.PutUint16(out[i*2:i*2+2], unit)
	}
	return out
}

func buildHeader(numFATSectors, firstDirectorySector, firstMiniFATSector, numMiniFATSectors uint32) []byte {
	header := make([]byte, 512)
	copy(header[:8], compoundSignature)
	binary.LittleEndian.PutUint16(header[24:26], 0x003E)
	binary.LittleEndian.PutUint16(header[26:28], 0x0003)
	binary.LittleEndian.PutUint16(header[28:30], 0xFFFE)
	binary.LittleEndian.PutUint16(header[30:32], writerSectorShift)
	binary.LittleEndian.PutUint16(header[32:34], writerMiniSectorShift)
	binary.LittleEndian.PutUint32(header[44:48], numFATSectors)
	binary.LittleEndian.PutUint32(header[48:52], firstDirectorySector)
	binary.LittleEndian.PutUint32(header[56:60], writerMiniStreamCutoff)
	binary.LittleEndian.PutUint32(header[60:64], firstMiniFATSector)
	binary.LittleEndian.PutUint32(header[64:68], numMiniFATSectors)
	binary.LittleEndian.PutUint32(header[68:72], sectorEnd)
	binary.LittleEndian.PutUint32(header[72:76], 0)
	for i := uint32(0); i < numFATSectors; i++ {
		binary.LittleEndian.PutUint32(header[76+i*4:80+i*4], i)
	}
	for offset := 76 + int(numFATSectors)*4; offset < 512; offset += 4 {
		binary.LittleEndian.PutUint32(header[offset:offset+4], sectorFree)
	}
	return header
}

func linkChain(fat []uint32, sectors []uint32) {
	if len(sectors) == 0 {
		return
	}
	for i, sector := range sectors {
		if i+1 < len(sectors) {
			fat[sector] = sectors[i+1]
		} else {
			fat[sector] = sectorEnd
		}
	}
}

func sectorsNeeded(size, sectorSize int) int {
	if size <= 0 {
		return 0
	}
	return (size + sectorSize - 1) / sectorSize
}
