package branches

import "testing"

func TestBranches(t *testing.T) {
	for _, b := range []bool{true, false} {
		want := 2
		if b {
			want = 1
		}
		Initializations = 0
		for name, got := range map[string]int{
			"local": Choose(b), "initializer": Initialized(b),
			"named": Named(Flag(b)), "generic": Generic(Flag(b)),
			"shadowed": Shadowed(b, false),
		} {
			if got != want {
				t.Errorf("%s(%v): got %d, want %d", name, b, got, want)
			}
		}
		if Initializations != 1 {
			t.Fatal("initializer did not execute exactly once")
		}
	}
	if ImportOnly() != 1 {
		t.Fatal("wrong imported condition result")
	}
	Initializations = 0
	if InitOnly() != 1 || Initializations != 1 {
		t.Fatal("initializer effect missing")
	}
	Evaluations = 0
	if Effect() != 1 || Evaluations != 1 {
		t.Fatal("condition effect missing")
	}
}
