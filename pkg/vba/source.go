package vba

import (
	"bytes"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"unicode/utf16"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/vba/cfb"
)

const (
	dirStreamPath = "VBA/dir"
)

var unsafeFilenameChars = regexp.MustCompile(`[^A-Za-z0-9._ -]+`)

// SourceProject describes source modules parsed from vbaProject.bin.
type SourceProject struct {
	Family                    string                     `json:"family,omitempty"`
	PartURI                   string                     `json:"partUri,omitempty"`
	CodePage                  int                        `json:"codePage,omitempty"`
	ModuleCount               int                        `json:"moduleCount"`
	Modules                   []SourceModule             `json:"modules"`
	ProjectMetadata           *ProjectMetadata           `json:"projectMetadata,omitempty"`
	OfficeCompatibility       *OfficeCompatibilityReport `json:"officeCompatibility,omitempty"`
	HostCompatibilityWarnings []HostCompatibilityWarning `json:"hostCompatibilityWarnings,omitempty"`
	SignatureArtifacts        []SignatureArtifact        `json:"signatureArtifacts,omitempty"`
	Warnings                  []string                   `json:"warnings,omitempty"`
}

// OfficeCompatibilityReport separates package/source readability from proof
// that the target Office host will load the VBA project without repair.
type OfficeCompatibilityReport struct {
	OfficeLoadVerified bool                       `json:"officeLoadVerified"`
	Status             string                     `json:"status"`
	Risks              []HostCompatibilityWarning `json:"risks,omitempty"`
	Notes              []string                   `json:"notes,omitempty"`
}

// HostCompatibilityWarning describes a source-level VBA project shape that is
// package-valid but suspicious for the package host family.
type HostCompatibilityWarning struct {
	Code    string   `json:"code"`
	Message string   `json:"message"`
	Modules []string `json:"modules,omitempty"`
}

// SourceModule describes one VBA module stream and its extracted source.
type SourceModule struct {
	Number          int      `json:"number"`
	Name            string   `json:"name"`
	StreamName      string   `json:"streamName"`
	Kind            string   `json:"kind"`
	Extension       string   `json:"extension"`
	CodePage        int      `json:"codePage,omitempty"`
	SourceOffset    uint32   `json:"sourceOffset"`
	SourceBytes     int      `json:"sourceBytes,omitempty"`
	LineCount       int      `json:"lineCount,omitempty"`
	SHA256          string   `json:"sha256,omitempty"`
	SHA256Basis     string   `json:"sha256Basis,omitempty"`
	LineEnding      string   `json:"lineEnding,omitempty"`
	TrailingNewline bool     `json:"trailingNewline"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	Source          string   `json:"source,omitempty"`
	Warnings        []string `json:"warnings,omitempty"`
}

// ProjectMetadata describes useful text metadata from the CFB PROJECT stream.
type ProjectMetadata struct {
	StreamName       string                     `json:"streamName,omitempty"`
	Present          bool                       `json:"present"`
	LineCount        int                        `json:"lineCount,omitempty"`
	ID               string                     `json:"id,omitempty"`
	Name             string                     `json:"name,omitempty"`
	Modules          []ProjectModuleDeclaration `json:"modules,omitempty"`
	References       []ProjectReference         `json:"references,omitempty"`
	WorkspaceEntries []ProjectWorkspaceEntry    `json:"workspaceEntries,omitempty"`
	HasProjectWM     bool                       `json:"hasProjectWm,omitempty"`
	ProjectWMStream  string                     `json:"projectWmStream,omitempty"`
	Warnings         []string                   `json:"warnings,omitempty"`
}

// ProjectModuleDeclaration describes one PROJECT stream module declaration.
type ProjectModuleDeclaration struct {
	Kind  string `json:"kind"`
	Name  string `json:"name"`
	Value string `json:"value,omitempty"`
	Line  int    `json:"line"`
}

// ProjectReference describes one PROJECT stream reference-like declaration.
type ProjectReference struct {
	Kind  string `json:"kind"`
	Value string `json:"value"`
	Line  int    `json:"line"`
}

// ProjectWorkspaceEntry describes one PROJECT stream workspace entry.
type ProjectWorkspaceEntry struct {
	Name  string `json:"name"`
	Value string `json:"value"`
	Line  int    `json:"line"`
}

type dirModule struct {
	Name         string
	StreamName   string
	Kind         string
	SourceOffset uint32
}

// InspectSourceProject parses vbaProject.bin and extracts module source.
func InspectSourceProject(session opc.PackageSession) (*SourceProject, error) {
	info, data, err := inspectSourceProjectData(session)
	if err != nil {
		return nil, err
	}
	project, err := ParseSourceProjectForFamily(data, info.Family)
	if err != nil {
		return nil, err
	}
	if info.VBAProject != nil {
		project.PartURI = info.VBAProject.PartURI
	}
	project.SignatureArtifacts = append([]SignatureArtifact{}, info.SignatureArtifacts...)
	return project, nil
}

// ParseSourceProjectForFamily parses a standalone vbaProject.bin payload and
// annotates it with best-effort compatibility warnings for a target host family.
func ParseSourceProjectForFamily(data []byte, family string) (*SourceProject, error) {
	project, err := ParseSourceProject(data)
	if err != nil {
		return nil, err
	}
	project.Family = strings.ToLower(strings.TrimSpace(family))
	populateOfficeCompatibility(project)
	return project, nil
}

// SummarizeSourceProject returns a copy safe for command JSON output by
// preserving module metadata while omitting full source text.
func SummarizeSourceProject(project *SourceProject) *SourceProject {
	if project == nil {
		return nil
	}
	copyProject := *project
	copyProject.Modules = make([]SourceModule, 0, len(project.Modules))
	for _, module := range project.Modules {
		module.Source = ""
		copyProject.Modules = append(copyProject.Modules, module)
	}
	return &copyProject
}

func inspectSourceProjectData(session opc.PackageSession) (*Info, []byte, error) {
	info, err := Inspect(session)
	if err != nil {
		return nil, nil, err
	}
	if info.VBAProject == nil || !info.VBAProject.Exists {
		return info, nil, fmt.Errorf("package has no vbaProject.bin part")
	}
	data, err := session.ReadRawPart(info.VBAProject.PartURI)
	if err != nil {
		return info, nil, err
	}
	return info, data, nil
}

// HostCompatibilityWarnings reports source-level host-family risks that package
// validation alone cannot see. These checks are deliberately conservative: they
// only flag document-module names that are strongly associated with another
// Office host.
func HostCompatibilityWarnings(project *SourceProject) []HostCompatibilityWarning {
	if project == nil {
		return nil
	}
	family := strings.ToLower(strings.TrimSpace(project.Family))
	if family == "" {
		return nil
	}

	var excelDocModules []string
	var powerpointDocModules []string
	for _, module := range project.Modules {
		if !moduleIsDocumentLike(module) {
			continue
		}
		name := strings.TrimSpace(module.Name)
		switch {
		case isExcelDocumentModuleName(name):
			excelDocModules = append(excelDocModules, name)
		case isPowerPointDocumentModuleName(name):
			powerpointDocModules = append(powerpointDocModules, name)
		}
	}

	var warnings []HostCompatibilityWarning
	if family == "pptx" && len(excelDocModules) > 0 {
		warnings = append(warnings, HostCompatibilityWarning{
			Code:    "VBA_HOST_EXCEL_MODULES_IN_PPTM",
			Message: fmt.Sprintf("PowerPoint macro package contains Excel document module(s): %s. The package can be structurally valid while Office may repair or reject the VBA project; use a PowerPoint-native vbaProject.bin seed for PPTM outputs.", strings.Join(excelDocModules, ", ")),
			Modules: excelDocModules,
		})
	}
	if family == "xlsx" && len(powerpointDocModules) > 0 {
		warnings = append(warnings, HostCompatibilityWarning{
			Code:    "VBA_HOST_POWERPOINT_MODULES_IN_XLSM",
			Message: fmt.Sprintf("Excel macro package contains PowerPoint document-like module(s): %s. The package can be structurally valid while Office may repair or reject the VBA project; use an Excel-native vbaProject.bin seed for XLSM outputs.", strings.Join(powerpointDocModules, ", ")),
			Modules: powerpointDocModules,
		})
	}
	return warnings
}

func populateOfficeCompatibility(project *SourceProject) {
	if project == nil {
		return
	}
	hostWarnings := HostCompatibilityWarnings(project)
	project.HostCompatibilityWarnings = hostWarnings
	for _, warning := range hostWarnings {
		if !stringSliceContains(project.Warnings, warning.Message) {
			project.Warnings = append(project.Warnings, warning.Message)
		}
	}
	status := "unverified"
	if len(hostWarnings) > 0 {
		status = "risk"
	}
	project.OfficeCompatibility = &OfficeCompatibilityReport{
		OfficeLoadVerified: false,
		Status:             status,
		Risks:              hostWarnings,
		Notes: []string{
			"Package validation and source readback do not prove that Microsoft Office will load this VBA project without repair.",
		},
	}
}

func moduleIsDocumentLike(module SourceModule) bool {
	if strings.EqualFold(module.Kind, "class") {
		return true
	}
	return strings.EqualFold(module.Extension, ".cls")
}

func isExcelDocumentModuleName(name string) bool {
	normalized := strings.ToLower(strings.TrimSpace(name))
	if normalized == "thisworkbook" {
		return true
	}
	if strings.HasPrefix(normalized, "sheet") && allDigits(normalized[len("sheet"):]) {
		return true
	}
	if strings.HasPrefix(normalized, "chart") && allDigits(normalized[len("chart"):]) {
		return true
	}
	return false
}

func isPowerPointDocumentModuleName(name string) bool {
	normalized := strings.ToLower(strings.TrimSpace(name))
	if normalized == "thispresentation" {
		return true
	}
	return strings.HasPrefix(normalized, "slide") && allDigits(normalized[len("slide"):])
}

func allDigits(text string) bool {
	if text == "" {
		return false
	}
	for _, r := range text {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

func stringSliceContains(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}

func parseProjectMetadata(cfbFile *cfb.File, codePage int) *ProjectMetadata {
	metadata := &ProjectMetadata{}
	if projectWMPath := findCFBStreamPath(cfbFile.Streams(), "PROJECTwm"); projectWMPath != "" {
		metadata.HasProjectWM = true
		metadata.ProjectWMStream = projectWMPath
	}
	projectPath := findCFBStreamPath(cfbFile.Streams(), "PROJECT")
	if projectPath == "" {
		if metadata.HasProjectWM {
			metadata.Warnings = append(metadata.Warnings, "PROJECTwm stream exists but PROJECT stream was not found")
			return metadata
		}
		return nil
	}
	metadata.Present = true
	metadata.StreamName = projectPath
	data, err := cfbFile.Stream(projectPath)
	if err != nil {
		metadata.Warnings = append(metadata.Warnings, err.Error())
		return metadata
	}
	text := decodeMBCS(data, codePage)
	text = strings.ReplaceAll(text, "\r\n", "\n")
	text = strings.ReplaceAll(text, "\r", "\n")
	lines := strings.Split(text, "\n")
	if len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	metadata.LineCount = len(lines)
	inWorkspace := false
	for idx, line := range lines {
		lineNo := idx + 1
		trimmed := strings.TrimSpace(line)
		if trimmed == "" {
			continue
		}
		if strings.HasPrefix(trimmed, "[") && strings.HasSuffix(trimmed, "]") {
			inWorkspace = strings.EqualFold(trimmed, "[Workspace]")
			continue
		}
		key, value, ok := strings.Cut(trimmed, "=")
		if !ok {
			continue
		}
		key = strings.TrimSpace(key)
		value = strings.TrimSpace(value)
		lowerKey := strings.ToLower(key)
		if inWorkspace {
			metadata.WorkspaceEntries = append(metadata.WorkspaceEntries, ProjectWorkspaceEntry{Name: key, Value: value, Line: lineNo})
			continue
		}
		switch lowerKey {
		case "id":
			metadata.ID = strings.Trim(value, `"`)
		case "name":
			metadata.Name = strings.Trim(value, `"`)
		case "module", "class", "baseclass", "document":
			name := value
			if lowerKey == "document" {
				if before, _, ok := strings.Cut(value, "/"); ok {
					name = before
				}
			}
			metadata.Modules = append(metadata.Modules, ProjectModuleDeclaration{
				Kind:  lowerKey,
				Name:  strings.Trim(name, `"`),
				Value: value,
				Line:  lineNo,
			})
		case "reference", "object", "package", "control":
			metadata.References = append(metadata.References, ProjectReference{Kind: lowerKey, Value: value, Line: lineNo})
		}
	}
	return metadata
}

// ParseSourceProject parses a vbaProject.bin CFB payload.
func ParseSourceProject(data []byte) (*SourceProject, error) {
	cfbFile, err := cfb.Open(data)
	if err != nil {
		return nil, err
	}
	dirCompressed, err := cfbFile.Stream(dirStreamPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read VBA dir stream: %w", err)
	}
	dirData, err := DecompressContainer(dirCompressed)
	if err != nil {
		return nil, fmt.Errorf("failed to decompress VBA dir stream: %w", err)
	}
	codePage, modules, warnings, err := parseDirStream(dirData)
	if err != nil {
		return nil, err
	}

	project := &SourceProject{
		CodePage:    codePage,
		ModuleCount: len(modules),
		Warnings:    warnings,
	}
	project.ProjectMetadata = parseProjectMetadata(cfbFile, codePage)
	for idx, module := range modules {
		item := SourceModule{
			Number:       idx + 1,
			Name:         module.Name,
			StreamName:   module.StreamName,
			Kind:         module.Kind,
			Extension:    extensionForModuleKind(module.Kind),
			CodePage:     codePage,
			SourceOffset: module.SourceOffset,
		}
		if item.Name == "" {
			item.Name = item.StreamName
		}
		if item.Kind == "" {
			item.Kind = "unknown"
			item.Extension = ".bas"
			item.Warnings = append(item.Warnings, "module type was not present in dir stream")
		}
		streamPath := "VBA/" + item.StreamName
		streamData, err := cfbFile.Stream(streamPath)
		if err != nil {
			item.Warnings = append(item.Warnings, err.Error())
		} else if int(item.SourceOffset) > len(streamData) {
			item.Warnings = append(item.Warnings, fmt.Sprintf("source offset %d exceeds module stream size %d", item.SourceOffset, len(streamData)))
		} else {
			sourceCompressed := streamData[item.SourceOffset:]
			sourceBytes, err := DecompressContainer(sourceCompressed)
			if err != nil {
				item.Warnings = append(item.Warnings, "failed to decompress module source: "+err.Error())
			} else {
				source := decodeModuleSource(sourceBytes, codePage)
				item.Source = source
				item.SourceBytes = len([]byte(source))
				item.LineCount = countSourceLines(source)
				item.LineEnding = SourceLineEndingStyle(source)
				item.TrailingNewline = sourceHasTrailingLineEnding(source)
				sum := sha256.Sum256([]byte(source))
				item.SHA256 = hex.EncodeToString(sum[:])
				item.SHA256Basis = "decoded-source-utf8"
			}
		}
		project.Modules = append(project.Modules, withSourceModuleSelectors(item))
	}
	sort.SliceStable(project.Modules, func(i, j int) bool {
		return project.Modules[i].Number < project.Modules[j].Number
	})
	return project, nil
}

// DecompressContainer decompresses an MS-OVBA compressed stream container.
func DecompressContainer(data []byte) ([]byte, error) {
	if len(data) == 0 {
		return nil, fmt.Errorf("compressed container is empty")
	}
	if data[0] != 0x01 {
		return nil, fmt.Errorf("compressed container signature 0x%02x, want 0x01", data[0])
	}
	var out []byte
	pos := 1
	for pos < len(data) {
		if pos+2 > len(data) {
			return nil, fmt.Errorf("truncated compressed chunk header")
		}
		header := binary.LittleEndian.Uint16(data[pos : pos+2])
		if header == 0 {
			break
		}
		if header&0x7000 != 0x3000 {
			return nil, fmt.Errorf("invalid compressed chunk signature in header 0x%04x", header)
		}
		chunkSize := int(header&0x0FFF) + 3
		chunkEnd := pos + chunkSize
		if chunkEnd > len(data) {
			return nil, fmt.Errorf("compressed chunk exceeds stream size")
		}
		compressed := header&0x8000 != 0
		chunkData := data[pos+2 : chunkEnd]
		chunkStart := len(out)
		if !compressed {
			if len(chunkData) != 4096 {
				return nil, fmt.Errorf("raw compressed chunk has %d bytes, want 4096", len(chunkData))
			}
			out = append(out, chunkData...)
		} else if err := decompressChunk(chunkData, chunkStart, &out); err != nil {
			return nil, err
		}
		pos = chunkEnd
	}
	return out, nil
}

func decompressChunk(data []byte, chunkStart int, out *[]byte) error {
	pos := 0
	for pos < len(data) {
		flags := data[pos]
		pos++
		for bit := 0; bit < 8 && pos < len(data); bit++ {
			if flags&(1<<bit) == 0 {
				*out = append(*out, data[pos])
				pos++
				continue
			}
			if pos+2 > len(data) {
				return fmt.Errorf("truncated copy token")
			}
			token := binary.LittleEndian.Uint16(data[pos : pos+2])
			pos += 2
			offset, length := unpackCopyToken(token, len(*out)-chunkStart)
			copyStart := len(*out) - offset
			if copyStart < chunkStart || copyStart < 0 {
				return fmt.Errorf("copy token offset %d precedes decompressed chunk", offset)
			}
			for i := 0; i < length; i++ {
				*out = append(*out, (*out)[copyStart+i])
			}
		}
	}
	return nil
}

func unpackCopyToken(token uint16, difference int) (offset int, length int) {
	bitCount := 4
	limit := 16
	for difference > limit && bitCount < 12 {
		bitCount++
		limit <<= 1
	}
	lengthBits := 16 - bitCount
	lengthMask := uint16((1 << lengthBits) - 1)
	length = int(token&lengthMask) + 3
	offset = int(token>>lengthBits) + 1
	return offset, length
}

func parseDirStream(data []byte) (int, []dirModule, []string, error) {
	modulesRecord, err := findProjectModulesRecord(data)
	if err != nil {
		return 0, nil, nil, err
	}
	codePage, foundCodePage := findProjectCodePage(data, modulesRecord.recordStart)
	reader := dirReader{data: data, pos: modulesRecord.modulesStart, codePage: codePage}
	if err := reader.parseModules(modulesRecord.count); err != nil {
		return 0, nil, reader.warnings, err
	}
	if !foundCodePage {
		reader.warnings = append(reader.warnings, "PROJECTCODEPAGE record was not found before PROJECTMODULES; defaulted to Windows-1252")
	}
	return reader.codePage, reader.modules, reader.warnings, nil
}

type projectModulesRecord struct {
	recordStart  int
	countPayload int
	count        int
	modulesStart int
	modulesEnd   int
}

type dirReader struct {
	data     []byte
	pos      int
	codePage int
	modules  []dirModule
	warnings []string
}

func (r *dirReader) parseModules(count int) error {
	for moduleIndex := 0; moduleIndex < count; moduleIndex++ {
		module, err := r.parseModule()
		if err != nil {
			return err
		}
		r.modules = append(r.modules, module)
	}
	return nil
}

func (r *dirReader) parseModule() (dirModule, error) {
	var module dirModule
	for r.remaining() >= 2 {
		id := r.u16(r.pos)
		if id == 0x002B {
			if r.remaining() < 6 {
				return module, fmt.Errorf("module terminator is truncated")
			}
			r.pos += 6
			if module.StreamName == "" {
				module.StreamName = module.Name
				r.warnings = append(r.warnings, fmt.Sprintf("module %q did not include MODULESTREAMNAME", module.Name))
			}
			return module, nil
		}
		if r.remaining() < 6 {
			return module, fmt.Errorf("module record 0x%04x is truncated", id)
		}
		size := int(r.u32(r.pos + 2))
		payloadStart := r.pos + 6
		payloadEnd := payloadStart + size
		if payloadEnd > len(r.data) {
			return module, fmt.Errorf("module record 0x%04x exceeds dir stream size", id)
		}
		payload := r.data[payloadStart:payloadEnd]
		switch id {
		case 0x0019:
			module.Name = decodeMBCS(payload, r.codePage)
		case 0x0047:
			if name := decodeUTF16LE(payload); name != "" {
				module.Name = name
			}
		case 0x001A:
			module.StreamName = decodeMBCS(payload, r.codePage)
		case 0x0032:
			if name := decodeUTF16LE(payload); name != "" {
				module.StreamName = name
			}
		case 0x0031:
			if len(payload) < 4 {
				return module, fmt.Errorf("MODULEOFFSET record is too short")
			}
			module.SourceOffset = binary.LittleEndian.Uint32(payload[:4])
		case 0x0021:
			module.Kind = "standard"
		case 0x0022:
			module.Kind = "class"
		}
		r.pos = payloadEnd
	}
	return module, fmt.Errorf("module record terminated unexpectedly")
}

func findProjectModulesRecord(data []byte) (projectModulesRecord, error) {
	for pos := 0; pos+8 <= len(data); pos++ {
		if binary.LittleEndian.Uint16(data[pos:pos+2]) != 0x000F {
			continue
		}
		size := int(binary.LittleEndian.Uint32(data[pos+2 : pos+6]))
		if size != 2 {
			continue
		}
		count := int(binary.LittleEndian.Uint16(data[pos+6 : pos+8]))
		if count <= 0 {
			continue
		}
		modulesStart, err := skipProjectCookie(data, pos+8)
		if err != nil {
			continue
		}
		modulesEnd := modulesStart
		ok := true
		for moduleIndex := 0; moduleIndex < count; moduleIndex++ {
			_, blockEnd, err := readDirModuleBlock(data, modulesEnd)
			if err != nil {
				ok = false
				break
			}
			modulesEnd = blockEnd
		}
		if ok {
			return projectModulesRecord{
				recordStart:  pos,
				countPayload: pos + 6,
				count:        count,
				modulesStart: modulesStart,
				modulesEnd:   modulesEnd,
			}, nil
		}
	}
	return projectModulesRecord{}, fmt.Errorf("PROJECTMODULES record not found in VBA dir stream")
}

func findProjectCodePage(data []byte, end int) (int, bool) {
	if end > len(data) {
		end = len(data)
	}
	for pos := 0; pos+8 <= end; pos++ {
		if binary.LittleEndian.Uint16(data[pos:pos+2]) != 0x0003 {
			continue
		}
		if binary.LittleEndian.Uint32(data[pos+2:pos+6]) != 2 {
			continue
		}
		codePage := int(binary.LittleEndian.Uint16(data[pos+6 : pos+8]))
		if codePage > 0 {
			return codePage, true
		}
	}
	return 1252, false
}

func skipProjectCookie(data []byte, pos int) (int, error) {
	if len(data)-pos < 6 || binary.LittleEndian.Uint16(data[pos:pos+2]) != 0x0013 {
		return pos, nil
	}
	size := int(binary.LittleEndian.Uint32(data[pos+2 : pos+6]))
	recordEnd := pos + 6 + size
	if recordEnd > len(data) {
		return 0, fmt.Errorf("PROJECTCOOKIE record exceeds dir stream size")
	}
	return recordEnd, nil
}

func (r *dirReader) remaining() int {
	return len(r.data) - r.pos
}

func (r *dirReader) u16(offset int) uint16 {
	return binary.LittleEndian.Uint16(r.data[offset : offset+2])
}

func (r *dirReader) u32(offset int) uint32 {
	return binary.LittleEndian.Uint32(r.data[offset : offset+4])
}

func decodeModuleSource(data []byte, codePage int) string {
	data = bytes.TrimRight(data, "\x00")
	return decodeMBCS(data, codePage)
}

func decodeMBCS(data []byte, codePage int) string {
	if len(data) == 0 {
		return ""
	}
	// The common Office VBA path is Windows-1252 for ASCII-compatible source.
	// Keep non-ASCII bytes reversible enough for inspection without pulling in
	// code-page dependencies in this read-only slice.
	runes := make([]rune, 0, len(data))
	for _, b := range data {
		if b < 0x80 {
			runes = append(runes, rune(b))
			continue
		}
		if codePage == 65001 {
			return string(data)
		}
		runes = append(runes, rune(b))
	}
	return string(runes)
}

func decodeUTF16LE(data []byte) string {
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

func countSourceLines(source string) int {
	if source == "" {
		return 0
	}
	lines := strings.Count(source, "\n")
	if !strings.HasSuffix(source, "\n") {
		lines++
	}
	return lines
}

// SourceLineEndingStyle classifies line endings in decoded module source.
func SourceLineEndingStyle(source string) string {
	hasCRLF := false
	hasLF := false
	hasCR := false
	for i := 0; i < len(source); i++ {
		switch source[i] {
		case '\r':
			if i+1 < len(source) && source[i+1] == '\n' {
				hasCRLF = true
				i++
			} else {
				hasCR = true
			}
		case '\n':
			hasLF = true
		}
	}
	kinds := 0
	for _, present := range []bool{hasCRLF, hasLF, hasCR} {
		if present {
			kinds++
		}
	}
	switch {
	case kinds == 0:
		return "none"
	case kinds > 1:
		return "mixed"
	case hasCRLF:
		return "crlf"
	case hasLF:
		return "lf"
	default:
		return "cr"
	}
}

func sourceHasTrailingLineEnding(source string) bool {
	return strings.HasSuffix(source, "\n") || strings.HasSuffix(source, "\r")
}

func extensionForModuleKind(kind string) string {
	switch kind {
	case "class":
		return ".cls"
	default:
		return ".bas"
	}
}

func withSourceModuleSelectors(module SourceModule) SourceModule {
	builder := selectorBuilder{}
	if strings.TrimSpace(module.Name) != "" {
		module.PrimarySelector = "module:" + module.Name
	} else if module.Number > 0 {
		module.PrimarySelector = fmt.Sprintf("module:%d", module.Number)
	}
	builder.add(module.PrimarySelector)
	if module.Number > 0 {
		builder.add(fmt.Sprintf("module:%d", module.Number))
		builder.add(fmt.Sprintf("#%d", module.Number))
	}
	if strings.TrimSpace(module.Name) != "" {
		builder.add("module:" + module.Name)
		builder.add("name:" + module.Name)
		builder.add("~" + module.Name)
		builder.add(module.Name)
	}
	if strings.TrimSpace(module.StreamName) != "" {
		builder.add("stream:" + module.StreamName)
	}
	module.Selectors = builder.values
	return module
}

type selectorBuilder struct {
	values []string
	seen   map[string]bool
}

func (b *selectorBuilder) add(value string) {
	value = strings.TrimSpace(value)
	if value == "" {
		return
	}
	if b.seen == nil {
		b.seen = map[string]bool{}
	}
	key := strings.ToLower(value)
	if b.seen[key] {
		return
	}
	b.seen[key] = true
	b.values = append(b.values, value)
}

// ModuleOutputName returns a stable safe filename for an extracted module.
func ModuleOutputName(module SourceModule) string {
	name := module.Name
	if strings.TrimSpace(name) == "" {
		name = module.StreamName
	}
	if strings.TrimSpace(name) == "" {
		name = fmt.Sprintf("module-%d", module.Number)
	}
	name = unsafeFilenameChars.ReplaceAllString(name, "_")
	name = strings.Trim(name, " .")
	if name == "" {
		name = fmt.Sprintf("module-%d", module.Number)
	}
	ext := module.Extension
	if ext == "" {
		ext = extensionForModuleKind(module.Kind)
	}
	return filepath.Base(name) + ext
}
