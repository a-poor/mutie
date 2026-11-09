package consumer

import (
	numbers "example.com/mutie-go-numbers"
	"testing"
)

func TestImportedValue(t *testing.T) {
	if numbers.Checked() != 7 {
		t.Fatal("wrong imported value")
	}
}
