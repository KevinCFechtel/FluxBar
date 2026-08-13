//go:build !darwin

package standalone

import "github.com/KevinCFechtel/FluxBar/internal/model"

func initializeArticleHover() bool { return false }

func resetArticleHover() {}

func registerArticleHover(model.Entry) {}

func closeArticleHover() {}
