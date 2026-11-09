package godotenv_test

import (
	"testing"

	"github.com/joho/godotenv"
)

func TestNumericQuotedCRLFNormalization(t *testing.T) {
	values, err := godotenv.Unmarshal("VALUE=\"first\r\nsecond\r\nthird\"\r\n")
	if err != nil {
		t.Fatal(err)
	}
	if got := values["VALUE"]; got != "first\nsecond\nthird" {
		t.Fatalf("quoted CRLF normalization: got %q", got)
	}
}

func TestNumericUnterminatedValueAtEOF(t *testing.T) {
	_, err := godotenv.Unmarshal("VALUE=\"unfinished")
	if err == nil || err.Error() != `unterminated quoted value "unfinished` {
		t.Fatalf("unterminated value at EOF: %v", err)
	}
}

func TestNumericLiteralDollarPreserved(t *testing.T) {
	values, err := godotenv.Unmarshal("VALUE=\"$\"\n")
	if err != nil {
		t.Fatal(err)
	}
	if got := values["VALUE"]; got != "$" {
		t.Fatalf("literal dollar: got %q", got)
	}
}
