//go:build !darwin

package standalone

func darkAppearance() bool {
	return false
}

func observeAppearance(func(bool)) (func(), bool) {
	return func() {}, true
}
