package icons

import (
	"bytes"
	"encoding/base64"
	"errors"
	"fmt"
	"image"
	_ "image/gif"
	_ "image/jpeg"
	"image/png"
	"math"
	"strings"

	_ "github.com/Kodeworks/golang-image-ico"
	"github.com/srwiley/oksvg"
	"github.com/srwiley/rasterx"
	xdraw "golang.org/x/image/draw"
	_ "golang.org/x/image/webp"
)

const DefaultSize = 32

// DecodeDataURL decodes Miniflux icon data and returns the media type and bytes.
func DecodeDataURL(value string) (string, []byte, error) {
	header, payload, found := strings.Cut(strings.TrimSpace(value), ",")
	if !found || payload == "" {
		return "", nil, errors.New("ungültige Icon-Daten-URL")
	}
	header = strings.TrimSpace(header)
	if strings.HasPrefix(strings.ToLower(header), "data:") {
		header = header[len("data:"):]
	}
	parts := strings.Split(header, ";")
	if len(parts) < 2 || !strings.HasPrefix(strings.ToLower(parts[0]), "image/") {
		return "", nil, errors.New("Icon ist kein Bild")
	}
	base64Encoded := false
	for _, part := range parts[1:] {
		if strings.EqualFold(strings.TrimSpace(part), "base64") {
			base64Encoded = true
		}
	}
	if !base64Encoded {
		return "", nil, errors.New("Icon ist nicht base64-kodiert")
	}
	decoded, err := base64.StdEncoding.DecodeString(payload)
	if err != nil {
		return "", nil, fmt.Errorf("Icon kann nicht dekodiert werden: %w", err)
	}
	return strings.ToLower(parts[0]), decoded, nil
}

// NormalizeDataURL converts a Miniflux icon into a square PNG with transparent padding.
func NormalizeDataURL(value string, size int) ([]byte, error) {
	mediaType, data, err := DecodeDataURL(value)
	if err != nil {
		return nil, err
	}
	return Normalize(data, mediaType, size)
}

// Normalize converts a raster image or SVG into a square PNG.
func Normalize(data []byte, mediaType string, size int) ([]byte, error) {
	if size <= 0 {
		size = DefaultSize
	}
	if len(data) == 0 {
		return nil, errors.New("Icon ist leer")
	}

	var source image.Image
	var err error
	if strings.Contains(strings.ToLower(mediaType), "svg") || looksLikeSVG(data) {
		source, err = rasterizeSVG(data, size)
	} else {
		source, _, err = image.Decode(bytes.NewReader(data))
	}
	if err != nil {
		return nil, fmt.Errorf("Icon-Format kann nicht gelesen werden: %w", err)
	}
	return resizeAndEncode(source, size)
}

func looksLikeSVG(data []byte) bool {
	trimmed := strings.TrimSpace(string(data))
	return strings.HasPrefix(trimmed, "<svg") || strings.HasPrefix(trimmed, "<?xml")
}

func rasterizeSVG(data []byte, size int) (image.Image, error) {
	icon, err := oksvg.ReadIconStream(bytes.NewReader(data), oksvg.WarnErrorMode)
	if err != nil {
		return nil, err
	}
	width := icon.ViewBox.W
	height := icon.ViewBox.H
	if width <= 0 || height <= 0 {
		return nil, errors.New("SVG hat keine gültige Größe")
	}
	scale := math.Min(float64(size)/width, float64(size)/height)
	targetWidth := width * scale
	targetHeight := height * scale
	x := (float64(size) - targetWidth) / 2
	y := (float64(size) - targetHeight) / 2
	icon.SetTarget(x, y, targetWidth, targetHeight)

	destination := image.NewRGBA(image.Rect(0, 0, size, size))
	scanner := rasterx.NewScannerGV(size, size, destination, destination.Bounds())
	dasher := rasterx.NewDasher(size, size, scanner)
	icon.Draw(dasher, 1)
	return destination, nil
}

func resizeAndEncode(source image.Image, size int) ([]byte, error) {
	bounds := source.Bounds()
	if bounds.Dx() <= 0 || bounds.Dy() <= 0 {
		return nil, errors.New("Icon hat keine gültige Größe")
	}
	scale := math.Min(float64(size)/float64(bounds.Dx()), float64(size)/float64(bounds.Dy()))
	width := max(1, int(math.Round(float64(bounds.Dx())*scale)))
	height := max(1, int(math.Round(float64(bounds.Dy())*scale)))
	x := (size - width) / 2
	y := (size - height) / 2

	destination := image.NewRGBA(image.Rect(0, 0, size, size))
	xdraw.CatmullRom.Scale(destination, image.Rect(x, y, x+width, y+height), source, bounds, xdraw.Over, nil)
	return encodePNG(destination)
}

func encodePNG(source image.Image) ([]byte, error) {
	var output bytes.Buffer
	encoder := png.Encoder{CompressionLevel: png.BestSpeed}
	if err := encoder.Encode(&output, source); err != nil {
		return nil, err
	}
	return output.Bytes(), nil
}
