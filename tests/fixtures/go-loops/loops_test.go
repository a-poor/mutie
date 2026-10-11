package loops

import "testing"

func TestLoops(t *testing.T) {
	for name, got := range map[string]int{
		"count": Count(1), "local": Local(true), "import": ImportOnly(),
		"named": Named(true), "generic": Generic(Flag(true)),
		"shadowed": Shadowed(false, true), "initializer": InitOnly(),
	} {
		if got != 1 {
			t.Errorf("%s: got %d, want 1", name, got)
		}
	}
	if Count(0) != 0 || Local(false) != 0 {
		t.Fatal("empty loop returned a contribution")
	}
}

func TestEffects(t *testing.T) {
	i, c, b, u := Effects()
	if i != 1 || c != 3 || b != 2 || u != 2 {
		t.Fatalf("effects: got %d/%d/%d/%d; want 1/3/2/2", i, c, b, u)
	}
}
