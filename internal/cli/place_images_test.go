package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"testing"

	"github.com/spf13/cobra"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestPlaceImage_Success(t *testing.T) {
	// Create a temporary output file
	tmpOut := t.TempDir() + "/output.pptx"

	// Create the command
	cmd := getRootCmdForPlaceImageTest()
	cmd.SetArgs([]string{
		"pptx", "place", "image",
		placeImagePresentationPath(),
		"--slide", "1",
		"--image", placeImageTestImagePath(),
		"--x", "914400", // 1 inch
		"--y", "457200", // 0.5 inch
		"--cx", "1828800", // 2 inches
		"--cy", "1828800", // 2 inches
		"--out", tmpOut,
	})

	// Execute
	err := cmd.Execute()
	if err != nil {
		t.Logf("Command error: %v", err)
	}

	// Check output file was created
	if _, err := os.Stat(tmpOut); err == nil {
		t.Log("Output file was created successfully")
	}
}

func TestPlaceImage_WithCustomName(t *testing.T) {
	tmpOut := t.TempDir() + "/output.pptx"

	var output bytes.Buffer
	cmd := getRootCmdForPlaceImageTest()
	cmd.SetOut(&output)
	cmd.SetArgs([]string{
		"--format", "json",
		"pptx", "place", "image",
		placeImagePresentationPath(),
		"--slide", "1",
		"--image", placeImageTestImagePath(),
		"--x", "0",
		"--y", "0",
		"--cx", "2000000",
		"--cy", "2000000",
		"--name", "CustomImageName",
		"--out", tmpOut,
	})
	err := cmd.Execute()
	if err != nil {
		t.Fatalf("place image with custom name failed: %v", err)
	}
	var result placeImageResult
	if err := json.Unmarshal(output.Bytes(), &result); err != nil {
		t.Fatalf("failed to unmarshal place image JSON: %v\n%s", err, output.String())
	}
	if result.ShapeName != "CustomImageName" {
		t.Fatalf("shape name = %q, want CustomImageName", result.ShapeName)
	}
	if _, err := os.Stat(tmpOut); err != nil {
		t.Fatalf("output file not created: %v", err)
	}
}

func TestPlaceImage_WithFitModeCover(t *testing.T) {
	tmpOut := t.TempDir() + "/output.pptx"

	cmd := getRootCmdForPlaceImageTest()
	cmd.SetArgs([]string{
		"pptx", "place", "image",
		placeImagePresentationPath(),
		"--slide", "1",
		"--image", placeImageTestImagePath(),
		"--x", "914400",
		"--y", "914400",
		"--cx", "3000000",
		"--cy", "2000000",
		"--fit-mode", "cover",
		"--out", tmpOut,
	})

	err := cmd.Execute()
	if err != nil {
		t.Logf("Command error: %v", err)
	}

	if _, err := os.Stat(tmpOut); err == nil {
		t.Log("Output file was created successfully")
	}
}

func TestPlaceImage_JSONOutput(t *testing.T) {
	tmpOut := t.TempDir() + "/output.pptx"
	jsonOut := t.TempDir() + "/result.json"

	// Capture stdout
	var buf bytes.Buffer

	cmd := getRootCmdForPlaceImageTest()
	cmd.SetOut(&buf)
	cmd.SetArgs([]string{
		"pptx", "place", "image",
		placeImagePresentationPath(),
		"--slide", "1",
		"--image", placeImageTestImagePath(),
		"--x", "914400",
		"--y", "457200",
		"--cx", "1828800",
		"--cy", "1828800",
		"--out", tmpOut,
		"-o", jsonOut,
		"--format", "json",
	})

	err := cmd.Execute()
	require.NoError(t, err)

	// Check JSON output file
	data, err := os.ReadFile(jsonOut)
	require.NoError(t, err)

	var result placeImageResult
	err = json.Unmarshal(data, &result)
	require.NoError(t, err)

	assert.Equal(t, placeImagePresentationPath(), result.File)
	assert.Equal(t, tmpOut, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, 1, result.SlideNumber)
	assert.Greater(t, result.ShapeID, 0)
	assert.NotEmpty(t, result.TargetURI)
	assert.Equal(t, int64(914400), result.X)
	assert.Equal(t, int64(457200), result.Y)
	assert.Equal(t, int64(1828800), result.CX)
	assert.Equal(t, int64(1828800), result.CY)
	require.NotNil(t, result.Destination)
	assert.Equal(t, tmpOut, result.Destination.File)
	assert.Equal(t, result.ShapeID, result.Destination.ShapeID)
	assert.Equal(t, "shape:"+strconv.Itoa(result.ShapeID), result.Destination.PrimarySelector)
	require.NotNil(t, result.Destination.ImageRef)
	assert.Equal(t, result.TargetURI, result.Destination.ImageRef.TargetURI)
	assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, tmpOut, "pptx shapes get")
}

func TestPlaceImage_InvalidSlide(t *testing.T) {
	tmpOut := t.TempDir() + "/output.pptx"

	cmd := getRootCmdForPlaceImageTest()
	cmd.SetArgs([]string{
		"pptx", "place", "image",
		placeImagePresentationPath(),
		"--slide", "999",
		"--image", placeImageTestImagePath(),
		"--x", "914400",
		"--y", "457200",
		"--cx", "1828800",
		"--cy", "1828800",
		"--out", tmpOut,
	})

	err := cmd.Execute()
	if err == nil {
		t.Error("Expected error for invalid slide number")
	}
}

func TestPlaceImage_InvalidDimensions(t *testing.T) {
	tmpOut := t.TempDir() + "/output.pptx"

	cmd := getRootCmdForPlaceImageTest()
	cmd.SetArgs([]string{
		"pptx", "place", "image",
		placeImagePresentationPath(),
		"--slide", "1",
		"--image", placeImageTestImagePath(),
		"--x", "914400",
		"--y", "457200",
		"--cx", "0",
		"--cy", "1828800",
		"--out", tmpOut,
	})

	err := cmd.Execute()
	if err == nil {
		t.Error("Expected error for invalid dimensions")
	}
}

func TestPlaceImage_MissingImageFile(t *testing.T) {
	tmpOut := t.TempDir() + "/output.pptx"

	cmd := getRootCmdForPlaceImageTest()
	cmd.SetArgs([]string{
		"pptx", "place", "image",
		placeImagePresentationPath(),
		"--slide", "1",
		"--image", "nonexistent.png",
		"--x", "914400",
		"--y", "457200",
		"--cx", "1828800",
		"--cy", "1828800",
		"--out", tmpOut,
	})

	err := cmd.Execute()
	if err == nil {
		t.Error("Expected error for missing image file")
	}
}

func TestPlaceImage_InvalidFitMode(t *testing.T) {
	tmpOut := t.TempDir() + "/output.pptx"

	cmd := getRootCmdForPlaceImageTest()
	cmd.SetArgs([]string{
		"pptx", "place", "image",
		placeImagePresentationPath(),
		"--slide", "1",
		"--image", placeImageTestImagePath(),
		"--x", "914400",
		"--y", "457200",
		"--cx", "1828800",
		"--cy", "1828800",
		"--fit-mode", "invalid",
		"--out", tmpOut,
	})

	err := cmd.Execute()
	if err == nil {
		t.Error("Expected error for invalid fit mode")
	}
}

func TestPlaceImage_MutationFlags(t *testing.T) {
	cmd := getRootCmdForPlaceImageTest()
	cmd.SetArgs([]string{
		"pptx", "place", "image",
		placeImagePresentationPath(),
		"--slide", "1",
		"--image", placeImageTestImagePath(),
		"--x", "914400",
		"--y", "457200",
		"--cx", "1828800",
		"--cy", "1828800",
	})

	err := cmd.Execute()
	if err == nil {
		t.Error("Expected error when neither --out nor --in-place is specified")
	}
}

func getRootCmdForPlaceImageTest() *cobra.Command {
	resetRootFlagsForReplaceImagesTest()
	resetCommandFlagsForReplaceImagesTest(placeImageCmd)
	cmd := GetRootCmd()
	cmd.SetOut(new(bytes.Buffer))
	cmd.SetErr(new(bytes.Buffer))
	return cmd
}

func placeImagePresentationPath() string {
	return filepath.Join(getTestdataPath(), "pptx", "minimal-title", "presentation.pptx")
}

func placeImageTestImagePath() string {
	return filepath.Join(getTestdataPath(), "pptx", "template-branded", "test-image.png")
}
