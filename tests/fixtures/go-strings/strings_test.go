package literals

import "testing"

func TestPublicValues(t *testing.T) {
	_ = Weak()
	if Indexed() != 'a' || Divide() != 1 {
		t.Fatal("unexpected values")
	}
}
