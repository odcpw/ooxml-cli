package render

import (
	"fmt"
	"image"
	"os"

	// Register PNG and JPEG decoders for image.DecodeConfig.
	_ "image/jpeg"
	_ "image/png"
)

// ReadImageDimensions returns the pixel width and height of an image file by
// decoding only its header. It satisfies the ImageDimensions signature.
func ReadImageDimensions(path string) (int, int, error) {
	file, err := os.Open(path)
	if err != nil {
		return 0, 0, fmt.Errorf("failed to open image: %w", err)
	}
	defer file.Close()

	cfg, _, err := image.DecodeConfig(file)
	if err != nil {
		return 0, 0, fmt.Errorf("failed to decode image header: %w", err)
	}
	return cfg.Width, cfg.Height, nil
}
