package numbers

import "testing"

func TestPublicValues(t *testing.T) {
	_ = Weak()
	if Divide() != 10 || Indexed() != 9 {
		t.Fatal("unexpected values")
	}
}
