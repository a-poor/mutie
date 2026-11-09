package consumer

import (
	literals "example.com/mutie-go-strings"
	"testing"
)

func TestImportedValue(t *testing.T) {
	if literals.Checked() != "ready" {
		t.Fatal("wrong imported value")
	}
}
