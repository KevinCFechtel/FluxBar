//go:build compat
// +build compat

// icon-compat is a test-only helper invoked by Build/test-icon-compat.sh.
package main

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"os"

	"github.com/KevinCFechtel/FluxBar/internal/coreapi"
	"github.com/KevinCFechtel/FluxBar/internal/icons"
)

type fixture struct {
	Cases []caseInput `json:"cases"`
}

type caseInput struct {
	Name       string  `json:"name"`
	Width      int     `json:"width"`
	Height     int     `json:"height"`
	Background [4]byte `json:"background"`
	Foreground [4]byte `json:"foreground"`
	Inset      int     `json:"inset"`
	Malformed  bool    `json:"malformed"`
}

type decodedImage struct {
	Width  int    `json:"width"`
	Height int    `json:"height"`
	RGBA   string `json:"rgba"`
	PNG    string `json:"png"`
}

type caseOutput struct {
	Name     string          `json:"name"`
	Regular  *decodedImage   `json:"regular,omitempty"`
	Dark     *decodedImage   `json:"dark,omitempty"`
	Response json.RawMessage `json:"response"`
}

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: icon-compat <fixture.json>")
		os.Exit(2)
	}
	data, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	var input fixture
	if err := json.Unmarshal(data, &input); err != nil {
		panic(err)
	}

	outputs := make([]caseOutput, 0, len(input.Cases))
	for _, testCase := range input.Cases {
		dataURL := "malformed"
		if !testCase.Malformed {
			dataURL = syntheticDataURL(testCase)
		}
		regular, err := icons.NormalizeDataURL(dataURL, icons.DefaultSize)
		if err != nil {
			regular = nil
		}
		var dark []byte
		if len(regular) > 0 {
			dark, _, _ = icons.DarkModeVariant(regular, icons.BackgroundAuto)
		}
		wire, err := json.Marshal(coreapi.Response{
			OK:   true,
			Icon: &coreapi.Icon{Regular: regular, Dark: dark},
		})
		if err != nil {
			panic(err)
		}
		outputs = append(outputs, caseOutput{
			Name:     testCase.Name,
			Regular:  decodeImage(regular),
			Dark:     decodeImage(dark),
			Response: wire,
		})
	}
	if err := json.NewEncoder(os.Stdout).Encode(outputs); err != nil {
		panic(err)
	}
}

func syntheticDataURL(testCase caseInput) string {
	source := image.NewNRGBA(image.Rect(0, 0, testCase.Width, testCase.Height))
	background := color.NRGBA{R: testCase.Background[0], G: testCase.Background[1], B: testCase.Background[2], A: testCase.Background[3]}
	foreground := color.NRGBA{R: testCase.Foreground[0], G: testCase.Foreground[1], B: testCase.Foreground[2], A: testCase.Foreground[3]}
	for y := 0; y < testCase.Height; y++ {
		for x := 0; x < testCase.Width; x++ {
			pixel := background
			if x >= testCase.Inset && x < testCase.Width-testCase.Inset && y >= testCase.Inset && y < testCase.Height-testCase.Inset {
				pixel = foreground
			}
			source.SetNRGBA(x, y, pixel)
		}
	}
	var encoded bytes.Buffer
	if err := png.Encode(&encoded, source); err != nil {
		panic(err)
	}
	return "data:image/png;base64," + base64.StdEncoding.EncodeToString(encoded.Bytes())
}

func decodeImage(data []byte) *decodedImage {
	if len(data) == 0 {
		return nil
	}
	decoded, _, err := image.Decode(bytes.NewReader(data))
	if err != nil {
		panic(err)
	}
	bounds := decoded.Bounds()
	rgba := make([]byte, 0, bounds.Dx()*bounds.Dy()*4)
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			pixel := color.NRGBAModel.Convert(decoded.At(x, y)).(color.NRGBA)
			rgba = append(rgba, pixel.R, pixel.G, pixel.B, pixel.A)
		}
	}
	return &decodedImage{
		Width:  bounds.Dx(),
		Height: bounds.Dy(),
		RGBA:   base64.StdEncoding.EncodeToString(rgba),
		PNG:    base64.StdEncoding.EncodeToString(data),
	}
}
