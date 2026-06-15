package mutate

import (
	"errors"
	"strconv"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

const commentsFixture = "../../../testdata/pptx/title-content/presentation.pptx"

func TestAddComment_CreatesPartsAndRelationships(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()

	res, err := AddComment(&AddCommentRequest{
		Package:     pkg,
		SlideNumber: 1,
		Author:      "Alice",
		Initials:    "AB",
		Date:        "2026-06-06T10:30:00Z",
		Text:        "Fix the title",
	})
	require.NoError(t, err)
	assert.True(t, res.CreatedPart)
	assert.True(t, res.CreatedRelationship)
	assert.True(t, res.CreatedAuthorsPart)
	assert.True(t, res.CreatedAuthor)
	assert.Equal(t, "/ppt/comments/comment1.xml", res.CommentsPart)
	assert.Equal(t, 1, res.CommentID)
	assert.Equal(t, 0, res.AuthorID)

	// Content types registered.
	assert.Equal(t, namespaces.ContentTypeComments, pkg.GetContentType(res.CommentsPart))
	assert.Equal(t, namespaces.ContentTypeCommentAuthors, pkg.GetContentType(namespaces.CommentAuthorsPartURI))

	// Slide -> comments relationship.
	hasSlideRel := false
	for _, rel := range pkg.ListRelationships("/ppt/slides/slide1.xml") {
		if rel.Type == namespaces.RelComments {
			hasSlideRel = true
		}
	}
	assert.True(t, hasSlideRel, "expected slide->comments relationship")

	// presentation.xml -> commentAuthors relationship.
	hasAuthorsRel := false
	for _, rel := range pkg.ListRelationships(namespaces.PresentationPartURI) {
		if rel.Type == namespaces.RelCommentAuthors {
			hasAuthorsRel = true
		}
	}
	assert.True(t, hasAuthorsRel, "expected presentation->commentAuthors relationship")

	requirePackageValid(t, pkg)
}

func TestAddComment_SecondCommentSameAuthorReusesAndIncrements(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()

	first, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Text: "one"})
	require.NoError(t, err)
	second, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Text: "two"})
	require.NoError(t, err)

	assert.Equal(t, first.AuthorID, second.AuthorID, "same author should reuse id")
	assert.False(t, second.CreatedAuthor)
	assert.False(t, second.CreatedAuthorsPart)
	assert.False(t, second.CreatedPart)
	assert.Equal(t, 1, first.CommentID)
	assert.Equal(t, 2, second.CommentID)
	requirePackageValid(t, pkg)
}

func TestAddComment_DistinctAuthorsGetUniqueSlideIdx(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()

	alice, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Text: "from alice"})
	require.NoError(t, err)
	bob, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Bob", Text: "from bob"})
	require.NoError(t, err)

	// Tool-created comments are allocated a slide-global idx (1, 2) so a fresh
	// deck stays unambiguous. (Pre-existing per-author duplicates are handled by
	// the compound (authorId, idx) lookup, exercised below.)
	assert.NotEqual(t, alice.AuthorID, bob.AuthorID)
	assert.Equal(t, 1, alice.CommentID)
	assert.Equal(t, 2, bob.CommentID)

	listing, err := inspect.ListSlideComments(pkg, "/ppt/slides/slide1.xml", 1)
	require.NoError(t, err)
	require.Len(t, listing.Comments, 2)
	assert.NotEqual(t, listing.Comments[0].ID, listing.Comments[1].ID)

	// Removing Bob's comment by id leaves Alice's intact.
	_, err = RemoveComment(&RemoveCommentRequest{Package: pkg, SlideNumber: 1, CommentID: bob.CommentID})
	require.NoError(t, err)
	listing, err = inspect.ListSlideComments(pkg, "/ppt/slides/slide1.xml", 1)
	require.NoError(t, err)
	require.Len(t, listing.Comments, 1)
	assert.Equal(t, "from alice", listing.Comments[0].Text)
	requirePackageValid(t, pkg)
}

// makePerAuthorIdxCollision builds the legacy real-Office shape: two p:cm on the
// same slide sharing idx=1 but under different authorId (Alice=0, Bob=1), with a
// matching cmAuthorLst. It reuses the production AddComment plumbing and then
// rewrites Bob's idx from its slide-global value back to 1.
func makePerAuthorIdxCollision(t *testing.T, pkg opc.PackageSession) (slideURI string) {
	t.Helper()
	_, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Text: "from alice"})
	require.NoError(t, err)
	bob, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Bob", Text: "from bob"})
	require.NoError(t, err)

	slideURI = "/ppt/slides/slide1.xml"
	commentsURI, exists := inspect.FindSlideCommentsPart(pkg, slideURI)
	require.True(t, exists)
	doc, err := pkg.ReadXMLPart(commentsURI)
	require.NoError(t, err)
	root := doc.Root()

	// Force Bob's idx (slide-global 2) down to 1 to simulate per-author allocation.
	rewrote := false
	for _, cm := range namespaces.FindChildren(root, namespaces.NsP, "cm") {
		if cm.SelectAttrValue("authorId", "") == strconv.Itoa(bob.AuthorID) {
			cm.CreateAttr("idx", "1")
			rewrote = true
		}
	}
	require.True(t, rewrote, "expected to rewrite Bob's comment idx")
	require.NoError(t, pkg.ReplaceXMLPart(commentsURI, doc))

	// Verify the collision: two p:cm with idx=1 under authorId 0 and 1.
	listing, err := inspect.ListSlideComments(pkg, slideURI, 1)
	require.NoError(t, err)
	require.Len(t, listing.Comments, 2)
	for _, c := range listing.Comments {
		require.Equal(t, 1, c.ID, "both comments must share idx=1")
	}
	return slideURI
}

func TestComments_PerAuthorIdxCollision_RemoveDisambiguates(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()
	slideURI := makePerAuthorIdxCollision(t, pkg)

	// Ambiguous remove (no author-id) must be refused, not silently delete one.
	_, err := RemoveComment(&RemoveCommentRequest{Package: pkg, SlideNumber: 1, CommentID: 1})
	require.Error(t, err)
	assert.True(t, errors.Is(err, ErrCommentAmbiguous))
	listing, err := inspect.ListSlideComments(pkg, slideURI, 1)
	require.NoError(t, err)
	require.Len(t, listing.Comments, 2, "ambiguous remove must not delete anything")

	// Remove Bob's (authorId=1) comment by compound address; Alice's survives.
	res, err := RemoveComment(&RemoveCommentRequest{Package: pkg, SlideNumber: 1, CommentID: 1, AuthorID: 1, AuthorIDSet: true})
	require.NoError(t, err)
	assert.Equal(t, "from bob", res.PreviousText)
	listing, err = inspect.ListSlideComments(pkg, slideURI, 1)
	require.NoError(t, err)
	require.Len(t, listing.Comments, 1)
	assert.Equal(t, "from alice", listing.Comments[0].Text)
	requirePackageValid(t, pkg)
}

func TestComments_PerAuthorIdxCollision_EditDisambiguates(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()
	slideURI := makePerAuthorIdxCollision(t, pkg)

	// Ambiguous edit (no author-id) must be refused.
	_, err := EditComment(&EditCommentRequest{Package: pkg, SlideNumber: 1, CommentID: 1, Text: "x", TextSet: true})
	require.Error(t, err)
	assert.True(t, errors.Is(err, ErrCommentAmbiguous))

	// Both colliding comments are independently reachable by compound address.
	editAlice, err := EditComment(&EditCommentRequest{Package: pkg, SlideNumber: 1, CommentID: 1, AuthorID: 0, AuthorIDSet: true, Text: "alice edited", TextSet: true})
	require.NoError(t, err)
	assert.Equal(t, "from alice", editAlice.PreviousText)
	assert.Equal(t, 0, editAlice.AuthorID)

	editBob, err := EditComment(&EditCommentRequest{Package: pkg, SlideNumber: 1, CommentID: 1, AuthorID: 1, AuthorIDSet: true, Text: "bob edited", TextSet: true})
	require.NoError(t, err)
	assert.Equal(t, "from bob", editBob.PreviousText)
	assert.Equal(t, 1, editBob.AuthorID)

	// Verify each target mutated exactly, by author.
	listing, err := inspect.ListSlideComments(pkg, slideURI, 1)
	require.NoError(t, err)
	require.Len(t, listing.Comments, 2)
	byAuthor := map[int]string{}
	for _, c := range listing.Comments {
		byAuthor[c.AuthorID] = c.Text
	}
	assert.Equal(t, "alice edited", byAuthor[0])
	assert.Equal(t, "bob edited", byAuthor[1])
	requirePackageValid(t, pkg)
}

func TestAddComment_SlideOutOfRange(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()

	_, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 99, Author: "Alice", Text: "x"})
	require.Error(t, err)
	assert.True(t, errors.Is(err, ErrSlideOutOfRange))
}

func TestAddComment_RequiresAuthor(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()
	_, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Text: "x"})
	require.Error(t, err)
}

func TestEditComment_TextAuthorDate(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()

	add, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Date: "2026-01-01T00:00:00Z", Text: "old"})
	require.NoError(t, err)

	edit, err := EditComment(&EditCommentRequest{
		Package:     pkg,
		SlideNumber: 1,
		CommentID:   add.CommentID,
		Text:        "new",
		TextSet:     true,
		Author:      "Bob",
		AuthorSet:   true,
		Date:        "2026-02-02T00:00:00Z",
		DateSet:     true,
	})
	require.NoError(t, err)
	assert.Equal(t, "new", edit.Text)
	assert.Equal(t, "Bob", edit.Author)
	assert.Equal(t, "2026-02-02T00:00:00Z", edit.Date)
	assert.Equal(t, "old", edit.PreviousText)
	assert.NotEqual(t, edit.ContentHash, edit.PreviousHash)

	// Readback reflects edit and a new author entry exists.
	listing, err := inspect.ListSlideComments(pkg, "/ppt/slides/slide1.xml", 1)
	require.NoError(t, err)
	require.Len(t, listing.Comments, 1)
	assert.Equal(t, "Bob", listing.Comments[0].Author)
	assert.Equal(t, "new", listing.Comments[0].Text)
	requirePackageValid(t, pkg)
}

func TestEditComment_HashMismatch(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()

	add, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Text: "x"})
	require.NoError(t, err)

	_, err = EditComment(&EditCommentRequest{
		Package:      pkg,
		SlideNumber:  1,
		CommentID:    add.CommentID,
		ExpectedHash: "sha256:deadbeef",
		Text:         "y",
		TextSet:      true,
	})
	require.Error(t, err)
	assert.True(t, errors.Is(err, ErrCommentHashMismatch))
}

func TestEditComment_NotFound(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()
	_, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Text: "x"})
	require.NoError(t, err)
	_, err = EditComment(&EditCommentRequest{Package: pkg, SlideNumber: 1, CommentID: 999, Text: "y", TextSet: true})
	require.Error(t, err)
	assert.True(t, errors.Is(err, ErrCommentNotFound))
}

func TestRemoveComment_PreservesPartWhenOthersRemain(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()

	first, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Text: "one"})
	require.NoError(t, err)
	_, err = AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Text: "two"})
	require.NoError(t, err)

	res, err := RemoveComment(&RemoveCommentRequest{Package: pkg, SlideNumber: 1, CommentID: first.CommentID})
	require.NoError(t, err)
	assert.False(t, res.RemovedPart)
	assert.Equal(t, "one", res.PreviousText)

	listing, err := inspect.ListSlideComments(pkg, "/ppt/slides/slide1.xml", 1)
	require.NoError(t, err)
	require.Len(t, listing.Comments, 1)
	requirePackageValid(t, pkg)
}

func TestRemoveComment_DropsPartWhenEmpty(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()

	add, err := AddComment(&AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Text: "only"})
	require.NoError(t, err)

	res, err := RemoveComment(&RemoveCommentRequest{Package: pkg, SlideNumber: 1, CommentID: add.CommentID})
	require.NoError(t, err)
	assert.True(t, res.RemovedPart)

	// Comments part and its slide relationship are gone.
	_, exists := inspect.FindSlideCommentsPart(pkg, "/ppt/slides/slide1.xml")
	assert.False(t, exists)
	requirePackageValid(t, pkg)
}

func TestRemoveComment_NotFound(t *testing.T) {
	pkg := openMutatePackage(t, commentsFixture)
	defer pkg.Close()
	_, err := RemoveComment(&RemoveCommentRequest{Package: pkg, SlideNumber: 1, CommentID: 0})
	require.Error(t, err)
	assert.True(t, errors.Is(err, ErrCommentNotFound))
}

func TestNewCommentElement_SchemaOrder(t *testing.T) {
	cm := newCommentElement(2, 5, "2026-06-06T10:30:00Z", "hi")
	children := cm.ChildElements()
	require.Len(t, children, 2)
	assert.Equal(t, "pos", children[0].Tag)
	assert.Equal(t, "text", children[1].Tag)
	assert.Equal(t, "p", children[0].Space)
	assert.Equal(t, "2", cm.SelectAttrValue("authorId", ""))
	assert.Equal(t, "5", cm.SelectAttrValue("idx", ""))
}
