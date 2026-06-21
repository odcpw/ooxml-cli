package namespaces

// PPTX legacy comment part constants. The slice uses the widely-loadable legacy
// comment form (p:cmLst per slide + a shared p:cmAuthorLst) defined by
// ECMA-376, rather than the Microsoft 2018 threaded-comment extension. This is
// the only comment representation that both passes structural validation and
// loads in every PowerPoint version.
const (
	// ContentTypeComments is the content-type override for a per-slide
	// comments part (root p:cmLst).
	ContentTypeComments = "application/vnd.openxmlformats-officedocument.presentationml.comments+xml"

	// ContentTypeCommentAuthors is the content-type override for the shared
	// comment-authors part (root p:cmAuthorLst).
	ContentTypeCommentAuthors = "application/vnd.openxmlformats-officedocument.presentationml.commentAuthors+xml"

	// RelComments is the relationship type linking a slide to its comments part.
	RelComments = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"

	// RelCommentAuthors is the relationship type linking presentation.xml to the
	// shared comment-authors part.
	RelCommentAuthors = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/commentAuthors"

	// CommentAuthorsPartURI is the conventional location of the shared
	// comment-authors part.
	CommentAuthorsPartURI = "/ppt/commentAuthors.xml"

	// PresentationPartURI is the main presentation part that owns the
	// commentAuthors relationship.
	PresentationPartURI = "/ppt/presentation.xml"
)
