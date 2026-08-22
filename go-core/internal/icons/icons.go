package icons

import (
	"bytes"
	"crypto/sha256"
	"encoding/base64"
	"errors"
	"fmt"
	"image"
	"image/color"
	_ "image/gif"
	_ "image/jpeg"
	"image/png"
	"math"
	"net/http"
	"strings"

	_ "github.com/sergeymakinen/go-ico"
	"github.com/srwiley/oksvg"
	"github.com/srwiley/rasterx"
	xdraw "golang.org/x/image/draw"
	_ "golang.org/x/image/webp"
)

const (
	DefaultSize                 = 32
	meaningfulTransparencyRatio = 0.10
)

// Diagnostic describes the processing stages of an icon without retaining its
// potentially large or sensitive image payload.
type Diagnostic struct {
	Stage             string
	DeclaredMediaType string
	DetectedMediaType string
	DecodedFormat     string
	Fingerprint       string
	SVGViewBox        string
	EncodedBytes      int
	DecodedBytes      int
	Width             int
	Height            int
	OutputBytes       int
	MeanLuminance     float64
	DarkContrast      float64
	LowContrastRatio  float64
	HasTransparency   bool
	TransparentRatio  float64
	ClassifiedDark    bool
	BackgroundMode    BackgroundMode
	BackgroundAdded   bool
}

type BackgroundMode string

const (
	BackgroundAuto   BackgroundMode = "auto"
	BackgroundAlways BackgroundMode = "always"
	BackgroundNever  BackgroundMode = "never"
)

// AppearanceAnalysis contains alpha-aware readability metrics for a dark menu.
type AppearanceAnalysis struct {
	MeanLuminance    float64
	DarkContrast     float64
	LowContrastRatio float64
	VisibleCoverage  float64
	HasTransparency  bool
	TransparentRatio float64
	ClassifiedDark   bool
	BackgroundMode   BackgroundMode
	BackgroundAdded  bool
}

// DecodeDataURL decodes Miniflux icon data and returns the media type and bytes.
func DecodeDataURL(value string) (string, []byte, error) {
	mediaType, data, _, err := decodeDataURL(value)
	return mediaType, data, err
}

func decodeDataURL(value string) (string, []byte, Diagnostic, error) {
	diagnostic := Diagnostic{Stage: "data_url"}
	header, payload, found := strings.Cut(strings.TrimSpace(value), ",")
	if !found || payload == "" {
		return "", nil, diagnostic, errors.New("ungültige Icon-Daten-URL")
	}
	diagnostic.EncodedBytes = len(payload)
	header = strings.TrimSpace(header)
	if strings.HasPrefix(strings.ToLower(header), "data:") {
		header = header[len("data:"):]
	}
	parts := strings.Split(header, ";")
	if len(parts) < 2 || !strings.HasPrefix(strings.ToLower(parts[0]), "image/") {
		return "", nil, diagnostic, errors.New("Icon ist kein Bild")
	}
	diagnostic.DeclaredMediaType = strings.ToLower(strings.TrimSpace(parts[0]))
	base64Encoded := false
	for _, part := range parts[1:] {
		if strings.EqualFold(strings.TrimSpace(part), "base64") {
			base64Encoded = true
		}
	}
	if !base64Encoded {
		return "", nil, diagnostic, errors.New("Icon ist nicht base64-kodiert")
	}
	diagnostic.Stage = "base64"
	decoded, err := base64.StdEncoding.DecodeString(payload)
	if err != nil {
		return "", nil, diagnostic, fmt.Errorf("Icon kann nicht dekodiert werden: %w", err)
	}
	diagnostic.DecodedBytes = len(decoded)
	diagnostic.DetectedMediaType = DetectMediaType(decoded)
	fingerprint := sha256.Sum256(decoded)
	diagnostic.Fingerprint = fmt.Sprintf("%x", fingerprint[:6])
	return diagnostic.DeclaredMediaType, decoded, diagnostic, nil
}

// NormalizeDataURL converts a Miniflux icon into a square PNG with transparent padding.
func NormalizeDataURL(value string, size int) ([]byte, error) {
	icon, _, err := NormalizeDataURLWithDiagnostic(value, size)
	return icon, err
}

// NormalizeDataURLWithDiagnostic converts an icon and reports exactly which
// processing stage and format were encountered.
func NormalizeDataURLWithDiagnostic(value string, size int) ([]byte, Diagnostic, error) {
	mediaType, data, diagnostic, err := decodeDataURL(value)
	if err != nil {
		return nil, diagnostic, err
	}
	return normalize(data, mediaType, size, diagnostic)
}

// Normalize converts a raster image or SVG into a square PNG.
func Normalize(data []byte, mediaType string, size int) ([]byte, error) {
	icon, _, err := normalize(data, mediaType, size, Diagnostic{
		DeclaredMediaType: strings.ToLower(strings.TrimSpace(mediaType)),
		DetectedMediaType: DetectMediaType(data),
		DecodedBytes:      len(data),
	})
	return icon, err
}

func normalize(data []byte, mediaType string, size int, diagnostic Diagnostic) ([]byte, Diagnostic, error) {
	if size <= 0 {
		size = DefaultSize
	}
	if len(data) == 0 {
		diagnostic.Stage = "decode"
		return nil, diagnostic, errors.New("Icon ist leer")
	}

	var source image.Image
	var err error
	if strings.Contains(strings.ToLower(mediaType), "svg") || looksLikeSVG(data) {
		diagnostic.Stage = "svg"
		source, diagnostic.SVGViewBox, err = rasterizeSVG(data, size)
		diagnostic.DecodedFormat = "svg"
	} else {
		diagnostic.Stage = "decode"
		source, diagnostic.DecodedFormat, err = image.Decode(bytes.NewReader(data))
	}
	if err != nil {
		return nil, diagnostic, fmt.Errorf("Icon-Format kann nicht gelesen werden: %w", err)
	}
	diagnostic.Width = source.Bounds().Dx()
	diagnostic.Height = source.Bounds().Dy()
	diagnostic.Stage = "resize"
	icon, err := resizeAndEncode(source, size)
	if err != nil {
		return nil, diagnostic, err
	}
	diagnostic.Stage = "complete"
	diagnostic.OutputBytes = len(icon)
	return icon, diagnostic, nil
}

// DetectMediaType identifies common icon formats from their content instead of
// trusting the MIME type supplied by the feed.
func DetectMediaType(data []byte) string {
	if looksLikeSVG(data) {
		return "image/svg+xml"
	}
	if len(data) >= 4 && bytes.Equal(data[:4], []byte{0x00, 0x00, 0x01, 0x00}) {
		return "image/x-icon"
	}
	if len(data) >= 12 && string(data[4:8]) == "ftyp" {
		switch string(data[8:12]) {
		case "avif", "avis":
			return "image/avif"
		case "heic", "heix", "hevc", "hevx", "mif1", "msf1":
			return "image/heif"
		}
	}
	return strings.TrimSpace(strings.Split(http.DetectContentType(data), ";")[0])
}

func looksLikeSVG(data []byte) bool {
	trimmed := strings.TrimSpace(string(data))
	return strings.HasPrefix(trimmed, "<svg") || strings.HasPrefix(trimmed, "<?xml")
}

func rasterizeSVG(data []byte, size int) (image.Image, string, error) {
	icon, err := oksvg.ReadIconStream(bytes.NewReader(data), oksvg.WarnErrorMode)
	if err != nil {
		return nil, "", err
	}
	viewBox := fmt.Sprintf("%g %g %g %g", icon.ViewBox.X, icon.ViewBox.Y, icon.ViewBox.W, icon.ViewBox.H)
	width := icon.ViewBox.W
	height := icon.ViewBox.H
	if width <= 0 || height <= 0 {
		return nil, viewBox, errors.New("SVG hat keine gültige Größe")
	}
	scale := math.Min(float64(size)/width, float64(size)/height)
	targetWidth := width * scale
	targetHeight := height * scale
	x := (float64(size) - targetWidth) / 2
	y := (float64(size) - targetHeight) / 2
	// oksvg.SetTarget subtracts the viewBox origin before applying its scale.
	// With a non-zero origin that translation remains unscaled and can move the
	// whole drawing outside the canvas. Build the affine transform explicitly.
	icon.Transform = rasterx.Matrix2D{
		A: scale,
		D: scale,
		E: x - icon.ViewBox.X*scale,
		F: y - icon.ViewBox.Y*scale,
	}

	destination := image.NewRGBA(image.Rect(0, 0, size, size))
	scanner := rasterx.NewScannerGV(size, size, destination, destination.Bounds())
	dasher := rasterx.NewDasher(size, size, scanner)
	icon.Draw(dasher, 1)
	return destination, viewBox, nil
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

// DarkModeVariant analyzes a normalized icon and, when needed, returns a
// version placed on a light rounded surface for dark menus. A nil image means
// the original icon is already readable and should be used for both modes.
func DarkModeVariant(data []byte, mode BackgroundMode) ([]byte, AppearanceAnalysis, error) {
	if mode == "" {
		mode = BackgroundAuto
	}
	source, _, err := image.Decode(bytes.NewReader(data))
	if err != nil {
		return nil, AppearanceAnalysis{BackgroundMode: mode}, fmt.Errorf("normalisiertes Icon analysieren: %w", err)
	}
	analysis := analyzeAppearance(source)
	analysis.BackgroundMode = mode
	addBackground := analysis.ClassifiedDark && analysis.HasTransparency
	switch mode {
	case BackgroundAlways:
		addBackground = true
	case BackgroundNever:
		addBackground = false
	case BackgroundAuto:
	default:
		return nil, analysis, fmt.Errorf("unbekannter Hintergrundmodus %q", mode)
	}
	if !addBackground {
		return nil, analysis, nil
	}

	bounds := source.Bounds()
	if bounds.Dx() <= 0 || bounds.Dy() <= 0 {
		return nil, analysis, errors.New("Icon hat keine gültige Größe")
	}
	destination := image.NewNRGBA(image.Rect(0, 0, bounds.Dx(), bounds.Dy()))
	drawRoundedSurface(destination)
	padding := max(2, min(bounds.Dx(), bounds.Dy())/12)
	target := destination.Bounds().Inset(padding)
	xdraw.CatmullRom.Scale(destination, target, source, bounds, xdraw.Over, nil)
	variant, err := encodePNG(destination)
	if err != nil {
		return nil, analysis, fmt.Errorf("Dark-Mode-Icon kodieren: %w", err)
	}
	analysis.BackgroundAdded = true
	return variant, analysis, nil
}

func analyzeAppearance(source image.Image) AppearanceAnalysis {
	bounds := source.Bounds()
	pixelCount := bounds.Dx() * bounds.Dy()
	if pixelCount <= 0 {
		return AppearanceAnalysis{}
	}
	// Approximation of a dark, slightly translucent macOS menu background.
	darkBackground := linearSRGB(0.12)
	var weightSum, luminanceSum, contrastSum, lowContrastWeight float64
	transparentPixels := 0
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			r, g, b, a := source.At(x, y).RGBA()
			if a < 0xffff {
				transparentPixels++
			}
			if a == 0 {
				continue
			}
			alpha := float64(a) / 65535
			red := float64(r) / float64(a)
			green := float64(g) / float64(a)
			blue := float64(b) / float64(a)
			luminance := 0.2126*linearSRGB(red) + 0.7152*linearSRGB(green) + 0.0722*linearSRGB(blue)
			composited := alpha*luminance + (1-alpha)*darkBackground
			contrast := contrastRatio(composited, darkBackground)
			weightSum += alpha
			luminanceSum += luminance * alpha
			contrastSum += contrast * alpha
			if contrast < 2.25 {
				lowContrastWeight += alpha
			}
		}
	}
	transparentRatio := float64(transparentPixels) / float64(pixelCount)
	analysis := AppearanceAnalysis{
		// A small alpha fringe is common around otherwise opaque favicons. It
		// should not trigger a background intended for transparent logo artwork.
		HasTransparency:  transparentRatio >= meaningfulTransparencyRatio,
		TransparentRatio: transparentRatio,
	}
	if weightSum == 0 {
		return analysis
	}
	analysis.MeanLuminance = luminanceSum / weightSum
	analysis.DarkContrast = contrastSum / weightSum
	analysis.LowContrastRatio = lowContrastWeight / weightSum
	analysis.VisibleCoverage = weightSum / float64(pixelCount)
	analysis.ClassifiedDark = analysis.VisibleCoverage >= 0.01 &&
		analysis.MeanLuminance <= 0.32 &&
		analysis.LowContrastRatio >= 0.50
	return analysis
}

func linearSRGB(value float64) float64 {
	if value <= 0.04045 {
		return value / 12.92
	}
	return math.Pow((value+0.055)/1.055, 2.4)
}

func contrastRatio(first, second float64) float64 {
	if first < second {
		first, second = second, first
	}
	return (first + 0.05) / (second + 0.05)
}

func drawRoundedSurface(destination *image.NRGBA) {
	bounds := destination.Bounds()
	inset := 1.0
	left := float64(bounds.Min.X) + inset
	top := float64(bounds.Min.Y) + inset
	right := float64(bounds.Max.X) - inset
	bottom := float64(bounds.Max.Y) - inset
	radius := math.Max(3, float64(min(bounds.Dx(), bounds.Dy()))*0.20)
	background := color.NRGBA{R: 242, G: 242, B: 242, A: 235}
	const samples = 4
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			inside := 0
			for sy := range samples {
				for sx := range samples {
					px := float64(x) + (float64(sx)+0.5)/samples
					py := float64(y) + (float64(sy)+0.5)/samples
					if insideRoundedRectangle(px, py, left, top, right, bottom, radius) {
						inside++
					}
				}
			}
			if inside > 0 {
				pixel := background
				pixel.A = uint8(int(background.A) * inside / (samples * samples))
				destination.SetNRGBA(x, y, pixel)
			}
		}
	}
}

func insideRoundedRectangle(x, y, left, top, right, bottom, radius float64) bool {
	if x < left || x >= right || y < top || y >= bottom {
		return false
	}
	nearestX := math.Max(left+radius, math.Min(x, right-radius))
	nearestY := math.Max(top+radius, math.Min(y, bottom-radius))
	dx := x - nearestX
	dy := y - nearestY
	return dx*dx+dy*dy <= radius*radius
}
