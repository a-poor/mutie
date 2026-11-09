package unrelated

import "testing"

func TestUnrelated(t *testing.T) {
	if Message != "okay" {
		t.Fatal(Message)
	}
}
