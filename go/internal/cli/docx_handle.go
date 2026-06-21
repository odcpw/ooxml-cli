package cli

import (
	"errors"

	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	docxhandle "github.com/ooxml-cli/ooxml-cli/pkg/docx/handle"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// docxParagraphHandleString renders a paragraph handle from an injected/reused
// w14:paraId marker, or "" when no marker is present (so a result field stays
// empty rather than emitting a meaningless handle).
func docxParagraphHandleString(paraID string) string {
	if paraID == "" {
		return ""
	}
	return docxhandle.FormatParagraph(paraID)
}

// mapDOCXHandleError maps a typed docx handle error to the CLI exit-code
// taxonomy, mirroring mapXLSXHandleError / mapPPTXHandleError so agents see one
// stable behaviour across formats: a MALFORMED or FORMAT_MISMATCH handle is an
// invalid-args error; a STALE / SCOPE_STALE / AMBIGUOUS handle is a
// target-not-found error (the addressed object is gone or not uniquely
// resolvable). The typed code string is preserved verbatim in the message.
func mapDOCXHandleError(err error) error {
	if err == nil {
		return nil
	}
	switch {
	case docxhandle.IsCode(err, docxhandle.CodeMalformed),
		docxhandle.IsCode(err, docxhandle.CodeFormatMismatch):
		return docxHandleCLIError(err, ExitInvalidArgs)
	case docxhandle.IsCode(err, docxhandle.CodeScopeStale),
		docxhandle.IsCode(err, docxhandle.CodeStale),
		docxhandle.IsCode(err, docxhandle.CodeAmbiguous):
		return docxHandleCLIError(err, ExitTargetNotFound)
	default:
		return NewCLIErrorf(ExitUnexpected, "%v", err)
	}
}

func docxHandleCLIError(err error, exitCode int) *CLIError {
	var herr *docxhandle.Error
	if errors.As(err, &herr) && herr.Code != "" {
		return &CLIError{ExitCode: exitCode, Code: herr.Code, Message: err.Error()}
	}
	return &CLIError{ExitCode: exitCode, Message: err.Error()}
}

// resolveDOCXParagraphHandleBlock decodes a paragraph handle and resolves it to
// the CURRENT 1-based body block index of the paragraph carrying the matching
// w14:paraId marker. The block index can then drive the existing positional
// paragraph/block mutate commands, so the handle is authoritative for WHICH
// paragraph is targeted while the mutation machinery is reused unchanged.
//
// It returns a CLI error (already mapped) when the handle is not a paragraph
// handle, is malformed, or no longer resolves.
func resolveDOCXParagraphHandleBlock(pkg opc.PackageSession, handleStr string) (int, error) {
	h, err := docxhandle.Parse(handleStr)
	if err != nil {
		return 0, mapDOCXHandleError(err)
	}
	if h.Kind != docxhandle.KindParagraph {
		return 0, InvalidArgsError("--handle must be a paragraph handle (H:docx/pt:doc/para:m:<paraId>)")
	}
	documentURI, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		return 0, NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
	}
	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		return 0, NewCLIErrorf(ExitUnexpected, "failed to read document part %s: %v", documentURI, err)
	}
	bodyElem, err := docxbody.FindBody(doc.Root())
	if err != nil {
		return 0, NewCLIErrorf(ExitUnexpected, "%v", err)
	}
	index, _, rerr := docxhandle.ResolveParagraphBlock(bodyElem, h)
	if rerr != nil {
		return 0, mapDOCXHandleError(rerr)
	}
	return index, nil
}

// resolveDOCXCommentHandleID decodes a comment handle and returns its native
// w:id, verifying the comment still exists in the comments part (so a stale
// handle is rejected up front rather than silently falling through to a
// not-found from the mutate layer).
func resolveDOCXCommentHandleID(pkg opc.PackageSession, handleStr string) (int, error) {
	h, err := docxhandle.Parse(handleStr)
	if err != nil {
		return 0, mapDOCXHandleError(err)
	}
	if h.Kind != docxhandle.KindComment {
		return 0, InvalidArgsError("--handle must be a comment handle (H:docx/pt:doc/comment:n:<id>)")
	}
	documentURI, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		return 0, NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
	}
	commentsURI, exists := docxinspect.FindCommentsPart(pkg, documentURI)
	if !exists {
		return 0, mapDOCXHandleError(&docxhandle.Error{Code: docxhandle.CodeScopeStale, Handle: handleStr, Message: "document has no comments part"})
	}
	commentsDoc, err := pkg.ReadXMLPart(commentsURI)
	if err != nil {
		return 0, NewCLIErrorf(ExitUnexpected, "failed to read comments part %s: %v", commentsURI, err)
	}
	if _, rerr := docxhandle.ResolveComment(commentsDoc.Root(), h); rerr != nil {
		return 0, mapDOCXHandleError(rerr)
	}
	return h.CommentID, nil
}

// resolveDOCXStyleHandleID decodes a style handle and returns its native
// w:styleId, verifying the style still exists in the styles part.
func resolveDOCXStyleHandleID(pkg opc.PackageSession, handleStr string) (string, error) {
	h, err := docxhandle.Parse(handleStr)
	if err != nil {
		return "", mapDOCXHandleError(err)
	}
	if h.Kind != docxhandle.KindStyle {
		return "", InvalidArgsError("--handle must be a style handle (H:docx/pt:styles/style:n:<styleId>)")
	}
	stylesURI, err := docxinspect.FindStylesPart(pkg)
	if err != nil {
		return "", NewCLIErrorf(ExitUnexpected, "failed to find styles part: %v", err)
	}
	if stylesURI == "" {
		return "", mapDOCXHandleError(&docxhandle.Error{Code: docxhandle.CodeScopeStale, Handle: handleStr, Message: "document has no styles part"})
	}
	stylesDoc, err := pkg.ReadXMLPart(stylesURI)
	if err != nil {
		return "", NewCLIErrorf(ExitUnexpected, "failed to read styles part %s: %v", stylesURI, err)
	}
	if _, rerr := docxhandle.ResolveStyle(stylesDoc.Root(), h); rerr != nil {
		return "", mapDOCXHandleError(rerr)
	}
	return h.StyleID, nil
}
