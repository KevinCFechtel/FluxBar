package icons

import (
	"bytes"
	"encoding/base64"
	"encoding/binary"
	"image"
	"image/color"
	"image/png"
	"testing"

	"github.com/KevinCFechtel/FluxBar/internal/assets"
	ico "github.com/sergeymakinen/go-ico"
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

func TestNormalizeDiagnosticIdentifiesUnsupportedAVIF(t *testing.T) {
	data := []byte("0000ftypavifunsupported")
	value := "data:image/avif;base64," + base64.StdEncoding.EncodeToString(data)
	_, diagnostic, err := NormalizeDataURLWithDiagnostic(value, 32)
	if err == nil {
		t.Fatal("NormalizeDataURLWithDiagnostic() returned no error")
	}
	if diagnostic.Stage != "decode" {
		t.Fatalf("stage = %q", diagnostic.Stage)
	}
	if diagnostic.DeclaredMediaType != "image/avif" || diagnostic.DetectedMediaType != "image/avif" {
		t.Fatalf("media types = declared %q, detected %q", diagnostic.DeclaredMediaType, diagnostic.DetectedMediaType)
	}
	if diagnostic.DecodedBytes != len(data) || diagnostic.Fingerprint == "" {
		t.Fatalf("incomplete diagnostic: %#v", diagnostic)
	}
}

func TestNormalizeBMPBasedICO(t *testing.T) {
	data := bmpICO(t, 16)
	value := "data:image/x-icon;base64," + base64.StdEncoding.EncodeToString(data)
	got, diagnostic, err := NormalizeDataURLWithDiagnostic(value, 32)
	if err != nil {
		t.Fatal(err)
	}
	if diagnostic.DetectedMediaType != "image/x-icon" || diagnostic.DecodedFormat != "ico" {
		t.Fatalf("unexpected formats: %#v", diagnostic)
	}
	if diagnostic.Width != 16 || diagnostic.Height != 16 || diagnostic.Stage != "complete" {
		t.Fatalf("unexpected diagnostic: %#v", diagnostic)
	}
	decoded, err := png.Decode(bytes.NewReader(got))
	if err != nil {
		t.Fatal(err)
	}
	if decoded.Bounds() != image.Rect(0, 0, 32, 32) {
		t.Fatalf("normalized bounds = %v", decoded.Bounds())
	}
}

func TestNormalizeMultiResolutionICOSelectsLargestImage(t *testing.T) {
	var container bytes.Buffer
	if err := ico.EncodeAll(&container, []image.Image{
		image.NewRGBA(image.Rect(0, 0, 16, 16)),
		image.NewRGBA(image.Rect(0, 0, 256, 256)),
	}); err != nil {
		t.Fatal(err)
	}
	_, diagnostic, err := NormalizeDataURLWithDiagnostic(
		"data:image/x-icon;base64,"+base64.StdEncoding.EncodeToString(container.Bytes()),
		32,
	)
	if err != nil {
		t.Fatal(err)
	}
	if diagnostic.DecodedFormat != "ico" || diagnostic.Width != 256 || diagnostic.Height != 256 {
		t.Fatalf("unexpected diagnostic: %#v", diagnostic)
	}
}

func TestDarkModeVariantAddsSurfaceToDarkTransparentIcon(t *testing.T) {
	data := transparentIconPNG(t, color.NRGBA{A: 255})
	variant, analysis, err := DarkModeVariant(data, BackgroundAuto)
	if err != nil {
		t.Fatal(err)
	}
	if !analysis.ClassifiedDark || !analysis.HasTransparency || !analysis.BackgroundAdded || len(variant) == 0 {
		t.Fatalf("unexpected analysis: %#v", analysis)
	}
	if analysis.MeanLuminance > 0.01 || analysis.LowContrastRatio < 0.9 {
		t.Fatalf("unexpected contrast metrics: %#v", analysis)
	}
	decoded, err := png.Decode(bytes.NewReader(variant))
	if err != nil {
		t.Fatal(err)
	}
	if _, _, _, alpha := decoded.At(0, 0).RGBA(); alpha != 0 {
		t.Fatal("rounded surface corner is not transparent")
	}
	if _, _, _, alpha := decoded.At(16, 2).RGBA(); alpha == 0 {
		t.Fatal("light surface is missing")
	}
}

func TestDarkModeVariantLeavesOpaqueDarkIconUnchangedInAutoMode(t *testing.T) {
	data := opaqueIconPNG(t, color.NRGBA{R: 8, G: 8, B: 8, A: 255})
	variant, analysis, err := DarkModeVariant(data, BackgroundAuto)
	if err != nil {
		t.Fatal(err)
	}
	if !analysis.ClassifiedDark || analysis.HasTransparency || analysis.TransparentRatio != 0 {
		t.Fatalf("unexpected classification: %#v", analysis)
	}
	if analysis.BackgroundAdded || variant != nil {
		t.Fatalf("opaque icon received a background: variant=%d analysis=%#v", len(variant), analysis)
	}

	variant, analysis, err = DarkModeVariant(data, BackgroundAlways)
	if err != nil || len(variant) == 0 || !analysis.BackgroundAdded {
		t.Fatalf("always mode: variant=%d analysis=%#v error=%v", len(variant), analysis, err)
	}
}

func TestDarkModeVariantIgnoresSmallTransparentFringe(t *testing.T) {
	source := image.NewNRGBA(image.Rect(0, 0, 32, 32))
	for y := range 32 {
		for x := range 32 {
			source.SetNRGBA(x, y, color.NRGBA{R: 8, G: 8, B: 8, A: 255})
		}
	}
	// 64 of 1024 pixels are transparent, matching the roughly six percent
	// alpha fringe observed in Golem's otherwise opaque favicon.
	for y := range 2 {
		for x := range 32 {
			source.SetNRGBA(x, y, color.NRGBA{})
		}
	}
	data := encodeTestPNG(t, source)

	variant, analysis, err := DarkModeVariant(data, BackgroundAuto)
	if err != nil {
		t.Fatal(err)
	}
	if !analysis.ClassifiedDark || analysis.HasTransparency {
		t.Fatalf("unexpected classification: %#v", analysis)
	}
	if analysis.TransparentRatio < 0.06 || analysis.TransparentRatio > 0.07 {
		t.Fatalf("transparent ratio = %f", analysis.TransparentRatio)
	}
	if analysis.BackgroundAdded || variant != nil {
		t.Fatalf("small alpha fringe received a background: variant=%d analysis=%#v", len(variant), analysis)
	}
}

func TestDarkModeVariantLeavesBrightIconUnchanged(t *testing.T) {
	data := transparentIconPNG(t, color.NRGBA{R: 255, G: 255, B: 255, A: 255})
	variant, analysis, err := DarkModeVariant(data, BackgroundAuto)
	if err != nil {
		t.Fatal(err)
	}
	if analysis.ClassifiedDark || analysis.BackgroundAdded || variant != nil {
		t.Fatalf("unexpected analysis: %#v", analysis)
	}

	variant, analysis, err = DarkModeVariant(data, BackgroundAlways)
	if err != nil || len(variant) == 0 || !analysis.BackgroundAdded {
		t.Fatalf("always mode: variant=%d analysis=%#v error=%v", len(variant), analysis, err)
	}
}

func TestDarkModeVariantNeverOverride(t *testing.T) {
	data := transparentIconPNG(t, color.NRGBA{A: 255})
	variant, analysis, err := DarkModeVariant(data, BackgroundNever)
	if err != nil {
		t.Fatal(err)
	}
	if !analysis.ClassifiedDark || analysis.BackgroundAdded || variant != nil {
		t.Fatalf("unexpected analysis: %#v", analysis)
	}
}

func transparentIconPNG(t *testing.T, foreground color.NRGBA) []byte {
	t.Helper()
	source := image.NewNRGBA(image.Rect(0, 0, 32, 32))
	for y := 6; y < 26; y++ {
		for x := 6; x < 26; x++ {
			source.SetNRGBA(x, y, foreground)
		}
	}
	return encodeTestPNG(t, source)
}

func opaqueIconPNG(t *testing.T, foreground color.NRGBA) []byte {
	t.Helper()
	source := image.NewNRGBA(image.Rect(0, 0, 32, 32))
	for y := range 32 {
		for x := range 32 {
			source.SetNRGBA(x, y, foreground)
		}
	}
	return encodeTestPNG(t, source)
}

func encodeTestPNG(t *testing.T, source image.Image) []byte {
	t.Helper()
	var output bytes.Buffer
	if err := png.Encode(&output, source); err != nil {
		t.Fatal(err)
	}
	return output.Bytes()
}

func bmpICO(t *testing.T, size int) []byte {
	t.Helper()
	var dib bytes.Buffer
	write := func(value any) {
		t.Helper()
		if err := binary.Write(&dib, binary.LittleEndian, value); err != nil {
			t.Fatal(err)
		}
	}
	xorBytes := size * size * 4
	maskRowBytes := ((size + 31) / 32) * 4
	maskBytes := maskRowBytes * size
	write(uint32(40))
	write(int32(size))
	write(int32(size * 2)) // ICO DIB stores the image and mask heights together.
	write(uint16(1))
	write(uint16(32))
	write(uint32(0))
	write(uint32(xorBytes + maskBytes))
	write(int32(0))
	write(int32(0))
	write(uint32(0))
	write(uint32(0))
	for range size * size {
		dib.Write([]byte{0x20, 0x80, 0xF0, 0xFF}) // BGRA
	}
	dib.Write(make([]byte, maskBytes))

	var output bytes.Buffer
	writeOutput := func(value any) {
		t.Helper()
		if err := binary.Write(&output, binary.LittleEndian, value); err != nil {
			t.Fatal(err)
		}
	}
	writeOutput(uint16(0))
	writeOutput(uint16(1))
	writeOutput(uint16(1))
	output.Write([]byte{byte(size), byte(size), 0, 0})
	writeOutput(uint16(1))
	writeOutput(uint16(32))
	writeOutput(uint32(dib.Len()))
	writeOutput(uint32(22))
	output.Write(dib.Bytes())
	return output.Bytes()
}
