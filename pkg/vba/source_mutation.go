package vba

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"os"
	"strings"
	"unicode/utf16"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/vba/cfb"
)

// SourceMutationResult describes a source-level VBA module mutation.
type SourceMutationResult struct {
	Action              string       `json:"action"`
	Family              string       `json:"family,omitempty"`
	PartURI             string       `json:"partUri,omitempty"`
	Module              SourceModule `json:"module"`
	PreviousCount       int          `json:"previousCount,omitempty"`
	ModuleCount         int          `json:"moduleCount,omitempty"`
	PreviousSHA256      string       `json:"previousSha256,omitempty"`
	SHA256              string       `json:"sha256,omitempty"`
	SourceBytes         int          `json:"sourceBytes,omitempty"`
	LineCount           int          `json:"lineCount,omitempty"`
	Warnings            []string     `json:"warnings,omitempty"`
	PurgedCaches        bool         `json:"purgedCaches"`
	RecompilesOnOpen    bool         `json:"recompilesOnOpen"`
	OfficeLoadVerified  bool         `json:"officeLoadVerified"`
	CompatibilityStatus string       `json:"compatibilityStatus,omitempty"`
}

// SourceMutationOptions controls source-level VBA rewrites that are package
// valid but not proven Office-load safe.
type SourceMutationOptions struct {
	AllowExperimentalSourceRewrite bool
	SourceKind                     string
}

// AddModuleOptions describes a source-level VBA module addition request.
type AddModuleOptions struct {
	Name                           string
	Kind                           string
	ExpectModuleCount              int
	AllowExperimentalSourceRewrite bool
}

// ReplaceModuleSource replaces an existing module source stream inside a package.
func ReplaceModuleSource(session opc.PackageSession, selector string, source []byte, expectSHA256 string, opts SourceMutationOptions) (*SourceMutationResult, *SourceProject, error) {
	info, data, err := inspectSourceProjectData(session)
	if err != nil {
		return nil, nil, err
	}
	if len(info.SignatureArtifacts) > 0 {
		return nil, nil, fmt.Errorf("refusing to replace VBA module because known signature artifacts are present")
	}
	if info.VBAProject == nil || !info.VBAProject.Exists {
		return nil, nil, fmt.Errorf("package has no vbaProject.bin part")
	}
	mutation, rewritten, err := ReplaceModuleSourceInProjectData(data, selector, source, expectSHA256, opts)
	if err != nil {
		return nil, nil, err
	}
	mutation.Family = info.Family
	mutation.PartURI = info.VBAProject.PartURI
	if err := session.ReplaceRawPart(info.VBAProject.PartURI, rewritten, ContentTypeVBAProject); err != nil {
		return nil, nil, err
	}
	project, err := ParseSourceProject(rewritten)
	if err != nil {
		return nil, nil, err
	}
	project.Family = info.Family
	project.PartURI = info.VBAProject.PartURI
	project.SignatureArtifacts = append([]SignatureArtifact{}, info.SignatureArtifacts...)
	populateOfficeCompatibility(project)
	return mutation, project, nil
}

// AddModuleSource adds a new module source stream inside a package.
func AddModuleSource(session opc.PackageSession, source []byte, opts AddModuleOptions) (*SourceMutationResult, *SourceProject, error) {
	info, data, err := inspectSourceProjectData(session)
	if err != nil {
		return nil, nil, err
	}
	if len(info.SignatureArtifacts) > 0 {
		return nil, nil, fmt.Errorf("refusing to add VBA module because known signature artifacts are present")
	}
	if info.VBAProject == nil || !info.VBAProject.Exists {
		return nil, nil, fmt.Errorf("package has no vbaProject.bin part")
	}
	mutation, rewritten, err := AddModuleSourceInProjectData(data, source, opts)
	if err != nil {
		return nil, nil, err
	}
	mutation.Family = info.Family
	mutation.PartURI = info.VBAProject.PartURI
	if err := session.ReplaceRawPart(info.VBAProject.PartURI, rewritten, ContentTypeVBAProject); err != nil {
		return nil, nil, err
	}
	project, err := ParseSourceProject(rewritten)
	if err != nil {
		return nil, nil, err
	}
	project.Family = info.Family
	project.PartURI = info.VBAProject.PartURI
	project.SignatureArtifacts = append([]SignatureArtifact{}, info.SignatureArtifacts...)
	populateOfficeCompatibility(project)
	return mutation, project, nil
}

// RemoveModuleSource removes an existing module source stream from a package.
func RemoveModuleSource(session opc.PackageSession, selector string, expectSHA256 string, opts SourceMutationOptions) (*SourceMutationResult, *SourceProject, error) {
	info, data, err := inspectSourceProjectData(session)
	if err != nil {
		return nil, nil, err
	}
	if len(info.SignatureArtifacts) > 0 {
		return nil, nil, fmt.Errorf("refusing to remove VBA module because known signature artifacts are present")
	}
	if info.VBAProject == nil || !info.VBAProject.Exists {
		return nil, nil, fmt.Errorf("package has no vbaProject.bin part")
	}
	mutation, rewritten, err := RemoveModuleSourceInProjectData(data, selector, expectSHA256, opts)
	if err != nil {
		return nil, nil, err
	}
	mutation.Family = info.Family
	mutation.PartURI = info.VBAProject.PartURI
	if err := session.ReplaceRawPart(info.VBAProject.PartURI, rewritten, ContentTypeVBAProject); err != nil {
		return nil, nil, err
	}
	project, err := ParseSourceProject(rewritten)
	if err != nil {
		return nil, nil, err
	}
	project.Family = info.Family
	project.PartURI = info.VBAProject.PartURI
	project.SignatureArtifacts = append([]SignatureArtifact{}, info.SignatureArtifacts...)
	populateOfficeCompatibility(project)
	return mutation, project, nil
}

// ReplaceModuleSourceInProjectData replaces one existing module stream in a raw
// vbaProject.bin payload and returns the rewritten payload.
func ReplaceModuleSourceInProjectData(data []byte, selector string, source []byte, expectSHA256 string, opts SourceMutationOptions) (*SourceMutationResult, []byte, error) {
	project, err := ParseSourceProject(data)
	if err != nil {
		return nil, nil, err
	}
	module, err := selectSourceModule(project.Modules, selector)
	if err != nil {
		return nil, nil, err
	}
	expected := normalizeSHA256Guard(expectSHA256)
	if expected != "" && !strings.EqualFold(expected, module.SHA256) {
		return nil, nil, fmt.Errorf("VBA module source hash mismatch: expected %s but found %s", expected, module.SHA256)
	}
	if module.StreamName == "" {
		return nil, nil, fmt.Errorf("VBA module %q has no stream name", module.Name)
	}
	if err := validateReplacementModuleSource(module, source, opts.SourceKind); err != nil {
		return nil, nil, err
	}
	encodedSource, warnings, err := encodeModuleSource(source, module.CodePage)
	if err != nil {
		return nil, nil, err
	}
	normalizedHash := sourceSHA256(encodedSource, module.CodePage)
	if normalizedHash == module.SHA256 {
		unchanged := module
		unchanged.Source = ""
		return &SourceMutationResult{
			Action:              "replace-module",
			Module:              unchanged,
			PreviousSHA256:      module.SHA256,
			SHA256:              module.SHA256,
			SourceBytes:         module.SourceBytes,
			LineCount:           module.LineCount,
			Warnings:            append(warnings, "replacement source is unchanged; preserved original vbaProject.bin bytes"),
			PurgedCaches:        false,
			RecompilesOnOpen:    false,
			OfficeLoadVerified:  false,
			CompatibilityStatus: "unchanged",
		}, append([]byte(nil), data...), nil
	}

	cfbFile, err := cfb.Open(data)
	if err != nil {
		return nil, nil, err
	}
	if err := requireExperimentalSourceRewriteAllowed(project, cfbFile, opts.AllowExperimentalSourceRewrite); err != nil {
		return nil, nil, err
	}
	dirCompressed, err := cfbFile.Stream(dirStreamPath)
	if err != nil {
		return nil, nil, err
	}
	dirData, err := DecompressContainer(dirCompressed)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to decompress VBA dir stream: %w", err)
	}
	patchedDir, patchedOffsets, err := rewriteDirModuleOffset(dirData, module, 0)
	if err != nil {
		return nil, nil, err
	}
	if patchedOffsets != 1 {
		return nil, nil, fmt.Errorf("VBA dir stream patched %d MODULEOFFSET records for %s, want 1", patchedOffsets, module.Name)
	}
	compressed := CompressContainerLiterals(encodedSource)
	replacements := map[string][]byte{
		dirStreamPath: CompressContainerLiterals(patchedDir),
	}
	streamPath := "VBA/" + module.StreamName
	if moduleStreamData, err := cfbFile.Stream(streamPath); err != nil {
		return nil, nil, err
	} else if int(module.SourceOffset) > len(moduleStreamData) {
		return nil, nil, fmt.Errorf("source offset %d exceeds module stream %s size %d", module.SourceOffset, streamPath, len(moduleStreamData))
	}
	replacements[streamPath] = compressed
	deleteStreams := vbaCompiledCacheStreams(cfbFile.Streams())
	if len(deleteStreams) > 0 {
		warnings = append(warnings, fmt.Sprintf("removed %d VBA compiled cache stream(s)", len(deleteStreams)))
	}
	if module.SourceOffset > 0 {
		warnings = append(warnings, fmt.Sprintf("removed performance-cache prefix from edited module %s; untouched module streams were preserved", module.Name))
	}
	warnings = append(warnings, "rewrote edited module source at MODULEOFFSET 0; Office compatibility remains unverified")
	rewritten, err := cfb.RewriteStreamsWithDeletes(data, replacements, deleteStreams)
	if err != nil {
		return nil, nil, err
	}

	updatedProject, err := ParseSourceProject(rewritten)
	if err != nil {
		return nil, nil, err
	}
	updatedModule, err := selectSourceModule(updatedProject.Modules, module.PrimarySelector)
	if err != nil {
		return nil, nil, err
	}
	updatedModule.Source = ""
	module.Source = ""
	result := &SourceMutationResult{
		Action:              "replace-module",
		Module:              updatedModule,
		PreviousSHA256:      module.SHA256,
		SHA256:              updatedModule.SHA256,
		SourceBytes:         updatedModule.SourceBytes,
		LineCount:           updatedModule.LineCount,
		Warnings:            warnings,
		PurgedCaches:        true,
		RecompilesOnOpen:    true,
		OfficeLoadVerified:  false,
		CompatibilityStatus: "experimental",
	}
	return result, rewritten, nil
}

// AddModuleSourceInProjectData adds one new module stream to a raw vbaProject.bin
// payload and returns the rewritten payload.
func AddModuleSourceInProjectData(data []byte, source []byte, opts AddModuleOptions) (*SourceMutationResult, []byte, error) {
	project, err := ParseSourceProject(data)
	if err != nil {
		return nil, nil, err
	}
	if opts.ExpectModuleCount > 0 && opts.ExpectModuleCount != len(project.Modules) {
		return nil, nil, fmt.Errorf("VBA module count mismatch: expected %d but found %d", opts.ExpectModuleCount, len(project.Modules))
	}
	name, err := resolveAddedModuleName(source, opts.Name)
	if err != nil {
		return nil, nil, err
	}
	kind, err := normalizeAddedModuleKind(opts.Kind)
	if err != nil {
		return nil, nil, err
	}
	if err := validateAddedModuleName(name); err != nil {
		return nil, nil, err
	}
	for _, existing := range project.Modules {
		if strings.EqualFold(existing.Name, name) || strings.EqualFold(existing.StreamName, name) {
			return nil, nil, fmt.Errorf("VBA module %q already exists", name)
		}
	}

	cfbFile, err := cfb.Open(data)
	if err != nil {
		return nil, nil, err
	}
	if err := requireExperimentalSourceRewriteAllowed(project, cfbFile, opts.AllowExperimentalSourceRewrite); err != nil {
		return nil, nil, err
	}
	if hasVersionDependentProjectMetadata(cfbFile) {
		return nil, nil, fmt.Errorf("refusing to add VBA module because this Office-shaped project has version-dependent _VBA_PROJECT metadata that must be regenerated for module-set changes; create an Office-authored vbaProject.bin seed and attach it, or replace an existing module")
	}
	if _, err := cfbFile.Stream("VBA/" + name); err == nil {
		return nil, nil, fmt.Errorf("VBA module stream %q already exists", name)
	}
	dirCompressed, err := cfbFile.Stream(dirStreamPath)
	if err != nil {
		return nil, nil, err
	}
	dirData, err := DecompressContainer(dirCompressed)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to decompress VBA dir stream: %w", err)
	}
	addedModule := SourceModule{
		Number:          len(project.Modules) + 1,
		Name:            name,
		StreamName:      name,
		Kind:            kind,
		Extension:       extensionForModuleKind(kind),
		CodePage:        project.CodePage,
		SourceOffset:    0,
		PrimarySelector: "module:" + name,
	}
	addedModule = withSourceModuleSelectors(addedModule)
	addedDir, err := addDirModule(dirData, addedModule)
	if err != nil {
		return nil, nil, err
	}

	encodedSource, warnings, err := prepareAddedModuleSource(source, name, project.CodePage)
	if err != nil {
		return nil, nil, err
	}
	replacements := map[string][]byte{
		dirStreamPath: CompressContainerLiterals(addedDir),
	}
	if projectPath := findCFBStreamPath(cfbFile.Streams(), "PROJECT"); projectPath != "" {
		projectData, err := cfbFile.Stream(projectPath)
		if err != nil {
			return nil, nil, err
		}
		patchedProject, addedLines, err := addProjectStreamModuleLines(projectData, addedModule)
		if err != nil {
			return nil, nil, err
		}
		replacements[projectPath] = patchedProject
		warnings = append(warnings, fmt.Sprintf("added %d PROJECT stream line(s) for module %s", addedLines, name))
	} else {
		warnings = append(warnings, "PROJECT stream was not present; skipped project metadata update")
	}
	if projectWMPath := findCFBStreamPath(cfbFile.Streams(), "PROJECTwm"); projectWMPath != "" {
		projectWMData, err := cfbFile.Stream(projectWMPath)
		if err != nil {
			return nil, nil, err
		}
		patchedProjectWM, addedEntries, err := addProjectWMModuleEntry(projectWMData, addedModule)
		if err != nil {
			return nil, nil, err
		}
		replacements[projectWMPath] = patchedProjectWM
		warnings = append(warnings, fmt.Sprintf("added %d PROJECTwm entry(s) for module %s", addedEntries, name))
	}
	additions := map[string][]byte{
		"VBA/" + name: CompressContainerLiterals(encodedSource),
	}
	deleteStreams := vbaCompiledCacheStreams(cfbFile.Streams())
	if len(deleteStreams) > 0 {
		warnings = append(warnings, fmt.Sprintf("removed %d VBA compiled cache stream(s)", len(deleteStreams)))
	}
	warnings = append(warnings, "added module source at MODULEOFFSET 0; untouched module streams were preserved and Office compatibility remains unverified")
	rewritten, err := cfb.RewriteStreamsWithAddsAndDeletes(data, replacements, additions, deleteStreams)
	if err != nil {
		return nil, nil, err
	}

	updatedProject, err := ParseSourceProject(rewritten)
	if err != nil {
		return nil, nil, err
	}
	updatedModule, err := selectSourceModule(updatedProject.Modules, addedModule.PrimarySelector)
	if err != nil {
		return nil, nil, err
	}
	updatedModule.Source = ""
	result := &SourceMutationResult{
		Action:              "add-module",
		Module:              updatedModule,
		PreviousCount:       len(project.Modules),
		ModuleCount:         len(updatedProject.Modules),
		SHA256:              updatedModule.SHA256,
		SourceBytes:         updatedModule.SourceBytes,
		LineCount:           updatedModule.LineCount,
		Warnings:            warnings,
		PurgedCaches:        true,
		RecompilesOnOpen:    true,
		OfficeLoadVerified:  false,
		CompatibilityStatus: "experimental",
	}
	return result, rewritten, nil
}

// RemoveModuleSourceInProjectData removes one existing module stream from a raw
// vbaProject.bin payload and returns the rewritten payload.
func RemoveModuleSourceInProjectData(data []byte, selector string, expectSHA256 string, opts SourceMutationOptions) (*SourceMutationResult, []byte, error) {
	project, err := ParseSourceProject(data)
	if err != nil {
		return nil, nil, err
	}
	if len(project.Modules) <= 1 {
		return nil, nil, fmt.Errorf("refusing to remove the last VBA module; use vba remove to remove the whole macro project")
	}
	module, err := selectSourceModule(project.Modules, selector)
	if err != nil {
		return nil, nil, err
	}
	expected := normalizeSHA256Guard(expectSHA256)
	if expected != "" && !strings.EqualFold(expected, module.SHA256) {
		return nil, nil, fmt.Errorf("VBA module source hash mismatch: expected %s but found %s", expected, module.SHA256)
	}
	if module.StreamName == "" {
		return nil, nil, fmt.Errorf("VBA module %q has no stream name", module.Name)
	}
	for _, candidate := range project.Modules {
		if strings.EqualFold(candidate.PrimarySelector, module.PrimarySelector) {
			continue
		}
		if strings.EqualFold(candidate.StreamName, module.StreamName) {
			return nil, nil, fmt.Errorf("refusing to remove VBA module %q because stream %q is shared by another module", module.Name, module.StreamName)
		}
	}

	cfbFile, err := cfb.Open(data)
	if err != nil {
		return nil, nil, err
	}
	if err := requireExperimentalSourceRewriteAllowed(project, cfbFile, opts.AllowExperimentalSourceRewrite); err != nil {
		return nil, nil, err
	}
	if hasVersionDependentProjectMetadata(cfbFile) {
		return nil, nil, fmt.Errorf("refusing to remove VBA module because this Office-shaped project has version-dependent _VBA_PROJECT metadata that must be regenerated for module-set changes; remove the whole macro project with vba remove, or create an Office-authored vbaProject.bin seed and attach it")
	}
	dirCompressed, err := cfbFile.Stream(dirStreamPath)
	if err != nil {
		return nil, nil, err
	}
	dirData, err := DecompressContainer(dirCompressed)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to decompress VBA dir stream: %w", err)
	}
	removedDir, err := removeDirModule(dirData, module)
	if err != nil {
		return nil, nil, err
	}
	warnings := []string{}
	replacements := map[string][]byte{
		dirStreamPath: CompressContainerLiterals(removedDir),
	}
	if projectPath := findCFBStreamPath(cfbFile.Streams(), "PROJECT"); projectPath != "" {
		projectData, err := cfbFile.Stream(projectPath)
		if err != nil {
			return nil, nil, err
		}
		patchedProject, removedLines := removeProjectStreamModuleLines(projectData, module)
		if removedLines == 0 {
			return nil, nil, fmt.Errorf("PROJECT stream did not contain module entry lines for %s", module.Name)
		} else {
			replacements[projectPath] = patchedProject
			warnings = append(warnings, fmt.Sprintf("removed %d PROJECT stream line(s) for module %s", removedLines, module.Name))
		}
	} else {
		warnings = append(warnings, "PROJECT stream was not present; skipped project metadata cleanup")
	}
	if projectWMPath := findCFBStreamPath(cfbFile.Streams(), "PROJECTwm"); projectWMPath != "" {
		projectWMData, err := cfbFile.Stream(projectWMPath)
		if err != nil {
			return nil, nil, err
		}
		patchedProjectWM, removedEntries, err := removeProjectWMModuleEntry(projectWMData, module)
		if err != nil {
			return nil, nil, err
		}
		if removedEntries == 0 {
			return nil, nil, fmt.Errorf("PROJECTwm stream did not contain module entry for %s", module.Name)
		}
		replacements[projectWMPath] = patchedProjectWM
		warnings = append(warnings, fmt.Sprintf("removed %d PROJECTwm entry(s) for module %s", removedEntries, module.Name))
	}
	deleteStreams := append(vbaCompiledCacheStreams(cfbFile.Streams()), "VBA/"+module.StreamName)
	if len(deleteStreams) > 1 {
		warnings = append(warnings, fmt.Sprintf("removed %d VBA compiled cache stream(s)", len(deleteStreams)-1))
	}
	warnings = append(warnings, "removed module metadata and stream; remaining module streams were preserved and Office compatibility remains unverified")
	rewritten, err := cfb.RewriteStreamsWithDeletes(data, replacements, deleteStreams)
	if err != nil {
		return nil, nil, err
	}

	updatedProject, err := ParseSourceProject(rewritten)
	if err != nil {
		return nil, nil, err
	}
	if _, err := selectSourceModule(updatedProject.Modules, module.PrimarySelector); err == nil {
		return nil, nil, fmt.Errorf("VBA module %s still exists after removal", module.PrimarySelector)
	}
	module.Source = ""
	result := &SourceMutationResult{
		Action:              "remove-module",
		Module:              module,
		PreviousSHA256:      module.SHA256,
		SourceBytes:         module.SourceBytes,
		LineCount:           module.LineCount,
		Warnings:            warnings,
		PurgedCaches:        true,
		RecompilesOnOpen:    true,
		OfficeLoadVerified:  false,
		CompatibilityStatus: "experimental",
	}
	return result, rewritten, nil
}

// CompressContainerLiterals writes a valid MS-OVBA compressed container without
// copy tokens. Full 4096-byte chunks are emitted as raw chunks; partial literal
// chunks are split so their compressed chunk payload fits in the 12-bit chunk
// size field.
func CompressContainerLiterals(raw []byte) []byte {
	out := []byte{0x01}
	for len(raw) >= 4096 {
		header := uint16(0x3000 | 0x0FFF)
		out = binary.LittleEndian.AppendUint16(out, header)
		out = append(out, raw[:4096]...)
		raw = raw[4096:]
	}
	if len(raw) == 0 {
		return out
	}
	for len(raw) > 0 {
		literalLen := len(raw)
		if literalLen > 3600 {
			literalLen = 3600
		}
		literalChunk := raw[:literalLen]
		var chunk []byte
		for offset := 0; offset < len(literalChunk); {
			n := len(literalChunk) - offset
			if n > 8 {
				n = 8
			}
			chunk = append(chunk, 0x00)
			chunk = append(chunk, literalChunk[offset:offset+n]...)
			offset += n
		}
		header := uint16(len(chunk)-1) | 0x3000 | 0x8000
		out = binary.LittleEndian.AppendUint16(out, header)
		out = append(out, chunk...)
		raw = raw[literalLen:]
	}
	return out
}

func ReadModuleSourceFile(path string) ([]byte, error) {
	if strings.TrimSpace(path) == "" {
		return nil, fmt.Errorf("source path is required")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	if len(data) == 0 {
		return nil, fmt.Errorf("VBA source file is empty")
	}
	return data, nil
}

func selectSourceModule(modules []SourceModule, selector string) (SourceModule, error) {
	selector = strings.TrimSpace(selector)
	if selector == "" {
		return SourceModule{}, fmt.Errorf("module selector is required")
	}
	var matches []SourceModule
	for _, module := range modules {
		for _, candidate := range module.Selectors {
			if strings.EqualFold(candidate, selector) {
				matches = append(matches, module)
				break
			}
		}
	}
	switch len(matches) {
	case 0:
		return SourceModule{}, fmt.Errorf("VBA module not found: %s", selector)
	case 1:
		return matches[0], nil
	default:
		var selectors []string
		for _, module := range matches {
			selectors = append(selectors, module.PrimarySelector)
		}
		return SourceModule{}, fmt.Errorf("VBA module %q is ambiguous; use one of: %s", selector, strings.Join(selectors, ", "))
	}
}

func encodeModuleSource(source []byte, codePage int) ([]byte, []string, error) {
	text := normalizeVBALineEndings(string(source))
	var warnings []string
	if !strings.HasSuffix(text, "\r\n") {
		text += "\r\n"
		warnings = append(warnings, "appended trailing CRLF to VBA source")
	}
	if codePage == 65001 {
		return []byte(text), warnings, nil
	}
	out := make([]byte, 0, len(text))
	for _, r := range text {
		if r > 0xFF {
			return nil, nil, fmt.Errorf("VBA source contains character %q that cannot be encoded with code page %d", r, codePage)
		}
		out = append(out, byte(r))
	}
	return out, warnings, nil
}

func normalizeVBALineEndings(text string) string {
	text = strings.ReplaceAll(text, "\r\n", "\n")
	text = strings.ReplaceAll(text, "\r", "\n")
	return strings.ReplaceAll(text, "\n", "\r\n")
}

func normalizeSHA256Guard(value string) string {
	value = strings.TrimSpace(value)
	value = strings.TrimPrefix(value, "sha256:")
	return strings.ToLower(value)
}

func sourceSHA256(encodedSource []byte, codePage int) string {
	decoded := decodeModuleSource(encodedSource, codePage)
	sum := sha256.Sum256([]byte(decoded))
	return hex.EncodeToString(sum[:])
}

func requireExperimentalSourceRewriteAllowed(project *SourceProject, cfbFile *cfb.File, allowed bool) error {
	if allowed {
		return nil
	}
	var reasons []string
	for _, module := range project.Modules {
		if module.SourceOffset > 0 {
			reasons = append(reasons, fmt.Sprintf("module %s has non-zero MODULEOFFSET %d", module.Name, module.SourceOffset))
			break
		}
	}
	if caches := vbaCompiledCacheStreams(cfbFile.Streams()); len(caches) > 0 {
		reasons = append(reasons, fmt.Sprintf("%d compiled-cache stream(s) present", len(caches)))
	}
	if len(reasons) == 0 {
		return nil
	}
	return fmt.Errorf("experimental VBA source rewrite refused for Office-shaped project (%s); rerun with --allow-experimental-vba-source-rewrite after backing up and accepting that Office-load compatibility is not verified", strings.Join(reasons, "; "))
}

func hasVersionDependentProjectMetadata(cfbFile *cfb.File) bool {
	data, err := cfbFile.Stream("VBA/_VBA_PROJECT")
	if err != nil {
		return false
	}
	// Tiny synthetic fixtures use a two-byte placeholder. Real Office projects
	// carry version-dependent module metadata here; add/remove must not leave it
	// out of sync with dir/PROJECT/PROJECTwm.
	return len(data) > 16
}

func rewriteDirModuleOffset(data []byte, target SourceModule, offset uint32) ([]byte, int, error) {
	out := append([]byte(nil), data...)
	record, err := findProjectModulesRecord(out)
	if err != nil {
		return nil, 0, err
	}
	pos := record.modulesStart
	patched := 0
	for moduleIndex := 0; moduleIndex < record.count; moduleIndex++ {
		var currentModule dirModule
		for len(out)-pos >= 2 {
			recordID := binary.LittleEndian.Uint16(out[pos : pos+2])
			if recordID == 0x002B {
				if len(out)-pos < 6 {
					return nil, 0, fmt.Errorf("module terminator is truncated")
				}
				pos += 6
				break
			}
			if len(out)-pos < 6 {
				return nil, 0, fmt.Errorf("module record 0x%04x is truncated", recordID)
			}
			recordSize := int(binary.LittleEndian.Uint32(out[pos+2 : pos+6]))
			recordPayloadStart := pos + 6
			recordPayloadEnd := recordPayloadStart + recordSize
			if recordPayloadEnd > len(out) {
				return nil, 0, fmt.Errorf("module record 0x%04x exceeds dir stream size", recordID)
			}
			if recordID == 0x0031 && dirModuleMatchesSourceModule(currentModule, target) {
				if recordSize < 4 {
					return nil, 0, fmt.Errorf("MODULEOFFSET record is too short")
				}
				binary.LittleEndian.PutUint32(out[recordPayloadStart:recordPayloadStart+4], offset)
				patched++
			}
			switch recordID {
			case 0x0019:
				currentModule.Name = decodeMBCS(out[recordPayloadStart:recordPayloadEnd], 1252)
			case 0x0047:
				if name := decodeUTF16LE(out[recordPayloadStart:recordPayloadEnd]); name != "" {
					currentModule.Name = name
				}
			case 0x001A:
				currentModule.StreamName = decodeMBCS(out[recordPayloadStart:recordPayloadEnd], 1252)
			case 0x0032:
				if name := decodeUTF16LE(out[recordPayloadStart:recordPayloadEnd]); name != "" {
					currentModule.StreamName = name
				}
			}
			pos = recordPayloadEnd
		}
	}
	return out, patched, nil
}

func removeDirModule(data []byte, module SourceModule) ([]byte, error) {
	record, err := findProjectModulesRecord(data)
	if err != nil {
		return nil, err
	}
	if record.count <= 1 {
		return nil, fmt.Errorf("refusing to remove the last VBA module")
	}

	scan := record.modulesStart
	removeStart := -1
	removeEnd := -1
	for moduleIndex := 0; moduleIndex < record.count; moduleIndex++ {
		blockStart := scan
		dirModule, blockEnd, err := readDirModuleBlock(data, scan)
		if err != nil {
			return nil, err
		}
		if dirModuleMatchesSourceModule(dirModule, module) {
			removeStart = blockStart
			removeEnd = blockEnd
			break
		}
		scan = blockEnd
	}
	if removeStart < 0 {
		return nil, fmt.Errorf("VBA module %s was not found in PROJECTMODULES records", module.PrimarySelector)
	}

	out := make([]byte, 0, len(data)-(removeEnd-removeStart))
	out = append(out, data[:removeStart]...)
	out = append(out, data[removeEnd:]...)
	binary.LittleEndian.PutUint16(out[record.countPayload:record.countPayload+2], uint16(record.count-1))
	return out, nil
}

func addDirModule(data []byte, module SourceModule) ([]byte, error) {
	record, err := findProjectModulesRecord(data)
	if err != nil {
		return nil, err
	}

	moduleBlock := buildDirModuleBlock(module)
	out := make([]byte, 0, len(data)+len(moduleBlock))
	out = append(out, data[:record.modulesEnd]...)
	out = append(out, moduleBlock...)
	out = append(out, data[record.modulesEnd:]...)
	binary.LittleEndian.PutUint16(out[record.countPayload:record.countPayload+2], uint16(record.count+1))
	return out, nil
}

func buildDirModuleBlock(module SourceModule) []byte {
	nameBytes := []byte(module.Name)
	streamBytes := []byte(module.StreamName)
	var out []byte
	out = append(out, vbaDirRecord(0x0019, nameBytes)...)
	out = append(out, vbaDirRecord(0x0047, utf16LEBytes(module.Name))...)
	out = append(out, vbaDirRecord(0x001A, streamBytes)...)
	out = append(out, vbaDirRecord(0x0032, utf16LEBytes(module.StreamName))...)
	out = append(out, vbaDirRecord(0x001C, nil)...)
	out = append(out, vbaDirRecord(0x0048, nil)...)
	out = append(out, vbaDirRecord(0x0031, le32Bytes(0))...)
	out = append(out, vbaDirRecord(0x001E, le32Bytes(0))...)
	out = append(out, vbaDirRecord(0x002C, le16Bytes(0xFFFF))...)
	if module.Kind == "class" {
		out = append(out, vbaDirRecord(0x0022, nil)...)
	} else {
		out = append(out, vbaDirRecord(0x0021, nil)...)
	}
	out = append(out, vbaDirRecord(0x002B, nil)...)
	return out
}

func readDirModuleBlock(data []byte, pos int) (dirModule, int, error) {
	var module dirModule
	for len(data)-pos >= 2 {
		id := binary.LittleEndian.Uint16(data[pos : pos+2])
		if id == 0x002B {
			if len(data)-pos < 6 {
				return module, 0, fmt.Errorf("module terminator is truncated")
			}
			if module.StreamName == "" {
				module.StreamName = module.Name
			}
			return module, pos + 6, nil
		}
		if len(data)-pos < 6 {
			return module, 0, fmt.Errorf("module record 0x%04x is truncated", id)
		}
		size := int(binary.LittleEndian.Uint32(data[pos+2 : pos+6]))
		payloadStart := pos + 6
		payloadEnd := payloadStart + size
		if payloadEnd > len(data) {
			return module, 0, fmt.Errorf("module record 0x%04x exceeds dir stream size", id)
		}
		payload := data[payloadStart:payloadEnd]
		switch id {
		case 0x0019:
			module.Name = decodeMBCS(payload, 1252)
		case 0x0047:
			if name := decodeUTF16LE(payload); name != "" {
				module.Name = name
			}
		case 0x001A:
			module.StreamName = decodeMBCS(payload, 1252)
		case 0x0032:
			if name := decodeUTF16LE(payload); name != "" {
				module.StreamName = name
			}
		case 0x0031:
			if len(payload) < 4 {
				return module, 0, fmt.Errorf("MODULEOFFSET record is too short")
			}
			module.SourceOffset = binary.LittleEndian.Uint32(payload[:4])
		case 0x0021:
			module.Kind = "standard"
		case 0x0022:
			module.Kind = "class"
		}
		pos = payloadEnd
	}
	return module, 0, fmt.Errorf("module record terminated unexpectedly")
}

func vbaDirRecord(id uint16, payload []byte) []byte {
	out := make([]byte, 6+len(payload))
	binary.LittleEndian.PutUint16(out[:2], id)
	binary.LittleEndian.PutUint32(out[2:6], uint32(len(payload)))
	copy(out[6:], payload)
	return out
}

func le16Bytes(value uint16) []byte {
	out := make([]byte, 2)
	binary.LittleEndian.PutUint16(out, value)
	return out
}

func le32Bytes(value uint32) []byte {
	out := make([]byte, 4)
	binary.LittleEndian.PutUint32(out, value)
	return out
}

func utf16LEBytes(text string) []byte {
	units := utf16.Encode([]rune(text))
	out := make([]byte, len(units)*2)
	for i, unit := range units {
		binary.LittleEndian.PutUint16(out[i*2:i*2+2], unit)
	}
	return out
}

func dirModuleMatchesSourceModule(candidate dirModule, module SourceModule) bool {
	if module.StreamName != "" && strings.EqualFold(candidate.StreamName, module.StreamName) {
		return true
	}
	return module.Name != "" && strings.EqualFold(candidate.Name, module.Name)
}

func findCFBStreamPath(paths []string, want string) string {
	for _, path := range paths {
		if strings.EqualFold(strings.ReplaceAll(path, "\\", "/"), want) {
			return path
		}
	}
	return ""
}

func resolveAddedModuleName(source []byte, requested string) (string, error) {
	requested = strings.TrimSpace(requested)
	attrName := moduleAttributeName(source)
	if requested != "" {
		if attrName != "" && !strings.EqualFold(requested, attrName) {
			return "", fmt.Errorf("requested module name %q does not match Attribute VB_Name %q", requested, attrName)
		}
		return requested, nil
	}
	if attrName != "" {
		return attrName, nil
	}
	return "", fmt.Errorf("module name is required when source lacks Attribute VB_Name")
}

func moduleAttributeName(source []byte) string {
	text := strings.ReplaceAll(string(source), "\r\n", "\n")
	text = strings.ReplaceAll(text, "\r", "\n")
	for _, line := range strings.Split(text, "\n") {
		trimmed := strings.TrimSpace(line)
		if !strings.HasPrefix(strings.ToLower(trimmed), "attribute vb_name") {
			continue
		}
		_, value, ok := strings.Cut(trimmed, "=")
		if !ok {
			continue
		}
		return strings.Trim(strings.TrimSpace(value), `"`)
	}
	return ""
}

func normalizeAddedModuleKind(kind string) (string, error) {
	kind = strings.ToLower(strings.TrimSpace(kind))
	switch kind {
	case "", "standard", "bas", ".bas":
		return "standard", nil
	case "class", "cls", ".cls":
		return "class", nil
	default:
		return "", fmt.Errorf("invalid VBA module kind %q (must be standard or class)", kind)
	}
}

func validateReplacementModuleSource(target SourceModule, source []byte, sourceKind string) error {
	if attrName := moduleAttributeName(source); attrName != "" && !strings.EqualFold(attrName, target.Name) {
		return fmt.Errorf("replacement source Attribute VB_Name %q does not match target module %q", attrName, target.Name)
	}
	kind, err := normalizeReplacementModuleKind(sourceKind)
	if err != nil {
		return err
	}
	if kind != "" && !strings.EqualFold(kind, target.Kind) {
		return fmt.Errorf("replacement source kind %q is incompatible with target module %q kind %q", kind, target.Name, target.Kind)
	}
	return nil
}

func normalizeReplacementModuleKind(kind string) (string, error) {
	kind = strings.ToLower(strings.TrimSpace(kind))
	switch kind {
	case "":
		return "", nil
	case "standard", "bas", ".bas":
		return "standard", nil
	case "class", "cls", ".cls":
		return "class", nil
	default:
		return "", fmt.Errorf("invalid replacement VBA module kind %q (must be standard or class)", kind)
	}
}

func validateAddedModuleName(name string) error {
	name = strings.TrimSpace(name)
	if name == "" {
		return fmt.Errorf("module name is required")
	}
	if strings.ContainsAny(name, `/\:"[]`) {
		return fmt.Errorf("module name %q contains unsupported characters", name)
	}
	if len(utf16.Encode([]rune(name))) > 31 {
		return fmt.Errorf("module name %q is longer than 31 UTF-16 code units", name)
	}
	return nil
}

func prepareAddedModuleSource(source []byte, name string, codePage int) ([]byte, []string, error) {
	text := string(source)
	warnings := []string{}
	if moduleAttributeName(source) == "" {
		text = fmt.Sprintf("Attribute VB_Name = \"%s\"\r\n%s", name, text)
		warnings = append(warnings, "prepended Attribute VB_Name to VBA source")
	}
	encoded, encodeWarnings, err := encodeModuleSource([]byte(text), codePage)
	if err != nil {
		return nil, nil, err
	}
	warnings = append(warnings, encodeWarnings...)
	return encoded, warnings, nil
}

func removeProjectStreamModuleLines(data []byte, module SourceModule) ([]byte, int) {
	if len(data) == 0 {
		return data, 0
	}
	text := string(data)
	lineEnding := "\n"
	if strings.Contains(text, "\r\n") {
		lineEnding = "\r\n"
	}
	trailing := strings.HasSuffix(text, "\n")
	normalized := strings.ReplaceAll(text, "\r\n", "\n")
	normalized = strings.ReplaceAll(normalized, "\r", "\n")
	lines := strings.Split(normalized, "\n")
	if len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	var kept []string
	removed := 0
	for _, line := range lines {
		if isProjectStreamModuleLine(line, module) {
			removed++
			continue
		}
		kept = append(kept, line)
	}
	if removed == 0 {
		return data, 0
	}
	out := strings.Join(kept, lineEnding)
	if trailing || len(kept) > 0 {
		out += lineEnding
	}
	return []byte(out), removed
}

func addProjectStreamModuleLines(data []byte, module SourceModule) ([]byte, int, error) {
	text := string(data)
	lineEnding := "\n"
	if strings.Contains(text, "\r\n") {
		lineEnding = "\r\n"
	}
	trailing := strings.HasSuffix(text, "\n")
	normalized := strings.ReplaceAll(text, "\r\n", "\n")
	normalized = strings.ReplaceAll(normalized, "\r", "\n")
	lines := strings.Split(normalized, "\n")
	if len(lines) > 0 && lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	for _, line := range lines {
		if isProjectStreamModuleLine(line, module) {
			return nil, 0, fmt.Errorf("PROJECT stream already contains module entry for %s", module.Name)
		}
	}

	moduleLine := "Module=" + module.Name
	if module.Kind == "class" {
		moduleLine = "Class=" + module.Name
	}
	workspaceLine := module.Name + "=0, 0, 0, 0, C"

	insertAt := len(lines)
	for idx, line := range lines {
		trimmed := strings.TrimSpace(line)
		lower := strings.ToLower(trimmed)
		if lower == "[workspace]" || strings.HasPrefix(trimmed, "[") || strings.HasPrefix(lower, "name=") {
			insertAt = idx
			break
		}
		if projectStreamModuleDeclarationKey(line) {
			insertAt = idx + 1
		}
	}
	outLines := make([]string, 0, len(lines)+3)
	outLines = append(outLines, lines[:insertAt]...)
	outLines = append(outLines, moduleLine)
	outLines = append(outLines, lines[insertAt:]...)

	workspaceAt := -1
	workspaceEnd := len(outLines)
	for idx, line := range outLines {
		if !strings.EqualFold(strings.TrimSpace(line), "[Workspace]") {
			continue
		}
		workspaceAt = idx
		workspaceEnd = idx + 1
		for scan := idx + 1; scan < len(outLines); scan++ {
			if strings.HasPrefix(strings.TrimSpace(outLines[scan]), "[") {
				break
			}
			workspaceEnd = scan + 1
		}
		break
	}
	if workspaceAt >= 0 {
		outLines = append(outLines[:workspaceEnd], append([]string{workspaceLine}, outLines[workspaceEnd:]...)...)
	} else {
		outLines = append(outLines, "[Workspace]", workspaceLine)
	}

	out := strings.Join(outLines, lineEnding)
	if trailing || len(outLines) > 0 {
		out += lineEnding
	}
	return []byte(out), len(outLines) - len(lines), nil
}

func projectStreamModuleDeclarationKey(line string) bool {
	trimmed := strings.TrimSpace(line)
	if trimmed == "" || strings.HasPrefix(trimmed, "[") {
		return false
	}
	key, _, ok := strings.Cut(trimmed, "=")
	if !ok {
		return false
	}
	switch strings.ToLower(strings.TrimSpace(key)) {
	case "document", "module", "class", "baseclass":
		return true
	default:
		return false
	}
}

func isProjectStreamModuleLine(line string, module SourceModule) bool {
	trimmed := strings.TrimSpace(line)
	if trimmed == "" || strings.HasPrefix(trimmed, "[") {
		return false
	}
	key, value, ok := strings.Cut(trimmed, "=")
	if !ok {
		return false
	}
	key = strings.ToLower(strings.TrimSpace(key))
	value = strings.TrimSpace(value)
	names := []string{module.Name, module.StreamName}
	switch key {
	case "module", "class", "baseclass":
		return matchesAnyModuleName(value, names)
	case "document":
		if before, _, ok := strings.Cut(value, "/"); ok {
			value = before
		}
		return matchesAnyModuleName(value, names)
	default:
		return matchesAnyModuleName(strings.TrimSpace(key), names)
	}
}

type projectWMModuleEntry struct {
	Name        string
	DisplayName string
}

func addProjectWMModuleEntry(data []byte, module SourceModule) ([]byte, int, error) {
	entries, err := parseProjectWMModuleEntries(data)
	if err != nil {
		return nil, 0, err
	}
	for _, entry := range entries {
		if projectWMEntryMatchesModule(entry, module) {
			return nil, 0, fmt.Errorf("PROJECTwm stream already contains module entry for %s", module.Name)
		}
	}
	entries = append(entries, projectWMModuleEntry{Name: module.Name, DisplayName: module.Name})
	return buildProjectWMModuleEntries(entries), 1, nil
}

func removeProjectWMModuleEntry(data []byte, module SourceModule) ([]byte, int, error) {
	entries, err := parseProjectWMModuleEntries(data)
	if err != nil {
		return nil, 0, err
	}
	kept := make([]projectWMModuleEntry, 0, len(entries))
	removed := 0
	for _, entry := range entries {
		if projectWMEntryMatchesModule(entry, module) {
			removed++
			continue
		}
		kept = append(kept, entry)
	}
	if removed == 0 {
		return data, 0, nil
	}
	return buildProjectWMModuleEntries(kept), removed, nil
}

func parseProjectWMModuleEntries(data []byte) ([]projectWMModuleEntry, error) {
	var entries []projectWMModuleEntry
	pos := 0
	for pos < len(data) {
		if data[pos] == 0 {
			if pos+1 < len(data) && data[pos+1] == 0 {
				return entries, nil
			}
			return nil, fmt.Errorf("PROJECTwm stream has an empty module name at byte %d", pos)
		}
		nameStart := pos
		for pos < len(data) && data[pos] != 0 {
			pos++
		}
		if pos >= len(data) {
			return nil, fmt.Errorf("PROJECTwm stream module name is unterminated")
		}
		name := string(data[nameStart:pos])
		pos++

		displayStart := pos
		for {
			if pos+1 >= len(data) {
				return nil, fmt.Errorf("PROJECTwm stream display name for %s is unterminated", name)
			}
			if data[pos] == 0 && data[pos+1] == 0 {
				displayName := decodeUTF16LE(data[displayStart:pos])
				pos += 2
				entries = append(entries, projectWMModuleEntry{Name: name, DisplayName: displayName})
				break
			}
			pos += 2
		}
	}
	return entries, nil
}

func buildProjectWMModuleEntries(entries []projectWMModuleEntry) []byte {
	var out []byte
	for _, entry := range entries {
		name := entry.Name
		displayName := entry.DisplayName
		if displayName == "" {
			displayName = name
		}
		out = append(out, []byte(name)...)
		out = append(out, 0)
		out = append(out, utf16LEBytes(displayName)...)
		out = append(out, 0, 0)
	}
	out = append(out, 0, 0)
	return out
}

func projectWMEntryMatchesModule(entry projectWMModuleEntry, module SourceModule) bool {
	names := []string{module.Name, module.StreamName}
	return matchesAnyModuleName(entry.Name, names) || matchesAnyModuleName(entry.DisplayName, names)
}

func matchesAnyModuleName(value string, names []string) bool {
	value = strings.Trim(value, `"`)
	for _, name := range names {
		if strings.TrimSpace(name) != "" && strings.EqualFold(value, strings.TrimSpace(name)) {
			return true
		}
	}
	return false
}

func vbaCompiledCacheStreams(paths []string) []string {
	var deletes []string
	for _, path := range paths {
		normalized := strings.ReplaceAll(path, "\\", "/")
		lower := strings.ToLower(normalized)
		if strings.HasPrefix(lower, "vba/__srp_") {
			deletes = append(deletes, path)
		}
	}
	return deletes
}
