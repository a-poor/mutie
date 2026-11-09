package sjson_test

import (
	"testing"

	"github.com/tidwall/sjson"
)

func TestCompatibilityFalseValue(t *testing.T) {
	got, err := sjson.Set(`{"flag":true}`, "flag", false)
	if err != nil || got != `{"flag":false}` {
		t.Fatalf("setting false: got %q, error %v", got, err)
	}
}

func TestCompatibilityScalarToArray(t *testing.T) {
	got, err := sjson.Set(`null`, "0", 7)
	if err != nil || got != `[7]` {
		t.Fatalf("setting an array index on null: got %q, error %v", got, err)
	}
}

func TestCompatibilityOptimisticRawValue(t *testing.T) {
	input := []byte(`{"value":12345678901234567890}`)
	got, err := sjson.SetRawBytesOptions(input, "value", []byte(`{"x":1}`),
		&sjson.Options{Optimistic: true, ReplaceInPlace: true})
	if err != nil || string(got) != `{"value":{"x":1}}` {
		t.Fatalf("optimistic raw replacement: got %q, error %v", got, err)
	}
}

func TestCompatibilityOptimisticStringValue(t *testing.T) {
	input := []byte(`{"value":"long original"}`)
	got, err := sjson.SetBytesOptions(input, "value", "new",
		&sjson.Options{Optimistic: true, ReplaceInPlace: true})
	if err != nil || string(got) != `{"value":"new"}` {
		t.Fatalf("optimistic string replacement: got %q, error %v", got, err)
	}
}

func TestCompatibilityComplexDeleteDiagnostic(t *testing.T) {
	input := `{"values":[{"x":1},{"x":2}]}`
	got, err := sjson.Delete(input, "values.#.x")
	if err == nil || err.Error() == "" || got != input {
		t.Fatalf("unsupported deletion must explain failure and preserve input: got %q, error %v", got, err)
	}
}
