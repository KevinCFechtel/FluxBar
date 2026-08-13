package icons

import (
	"bytes"
	"encoding/base64"
	"image"
	"image/color"
	"image/png"
	"testing"

	"github.com/KevinCFechtel/FluxBar/internal/assets"
)

func TestDecodeDataURL(t *testing.T) {
	payload := base64.StdEncoding.EncodeToString([]byte("image"))
	for _, value := range []string{
		"data:image/png;base64," + payload,
		"image/png;base64," + payload,
	} {
		mediaType, got, err := DecodeDataURL(value)
		if err != nil || mediaType != "image/png" || string(got) != "image" {
			t.Fatalf("DecodeDataURL(%q) = %q, %q, %v", value, mediaType, got, err)
		}
	}
}

func TestNormalizeRasterImage(t *testing.T) {
	source := image.NewRGBA(image.Rect(0, 0, 8, 4))
	for y := range 4 {
		for x := range 8 {
			source.Set(x, y, color.RGBA{R: 255, A: 255})
		}
	}
	var input bytes.Buffer
	if err := png.Encode(&input, source); err != nil {
		t.Fatal(err)
	}

	got, err := Normalize(input.Bytes(), "image/png", 32)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := png.Decode(bytes.NewReader(got))
	if err != nil {
		t.Fatal(err)
	}
	if decoded.Bounds().Dx() != 32 || decoded.Bounds().Dy() != 32 {
		t.Fatalf("normalized bounds = %v", decoded.Bounds())
	}
	if _, _, _, alpha := decoded.At(0, 0).RGBA(); alpha != 0 {
		t.Fatal("expected transparent padding")
	}
}

func TestNormalizeSVG(t *testing.T) {
	data := []byte(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="red"/></svg>`)
	got, err := Normalize(data, "image/svg+xml", 32)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := png.Decode(bytes.NewReader(got))
	if err != nil {
		t.Fatal(err)
	}
	if decoded.Bounds() != image.Rect(0, 0, 32, 32) {
		t.Fatalf("normalized bounds = %v", decoded.Bounds())
	}
}

func TestNormalizeMenuBarAsset(t *testing.T) {
	got, err := Normalize(assets.MinifluxSVG(), "image/svg+xml", 44)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := png.Decode(bytes.NewReader(got))
	if err != nil {
		t.Fatal(err)
	}
	if decoded.Bounds() != image.Rect(0, 0, 44, 44) {
		t.Fatalf("normalized bounds = %v", decoded.Bounds())
	}
	nonTransparent := 0
	for y := decoded.Bounds().Min.Y; y < decoded.Bounds().Max.Y; y++ {
		for x := decoded.Bounds().Min.X; x < decoded.Bounds().Max.X; x++ {
			if _, _, _, alpha := decoded.At(x, y).RGBA(); alpha > 0 {
				nonTransparent++
			}
		}
	}
	if nonTransparent == 0 {
		t.Fatal("normalized menu bar asset contains no visible pixels")
	}
}
