package godotenv_test

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/joho/godotenv"
)

func TestStringQuotedCRLFNormalization(t *testing.T) {
	values, err := godotenv.Unmarshal("VALUE=\"first\r\nsecond\r\nthird\"\r\n")
	if err != nil {
		t.Fatal(err)
	}
	if got := values["VALUE"]; got != "first\nsecond\nthird" {
		t.Fatalf("quoted CRLF normalization: got %q", got)
	}
}

func TestStringEmptyValueAtEOF(t *testing.T) {
	values, err := godotenv.Unmarshal("VALUE=")
	if err != nil {
		t.Fatal(err)
	}
	if got, present := values["VALUE"]; !present || got != "" {
		t.Fatalf("empty value at EOF: got %#v", values)
	}
}

func TestStringLiteralDollarPreserved(t *testing.T) {
	values, err := godotenv.Unmarshal("VALUE=\"$\"\n")
	if err != nil {
		t.Fatal(err)
	}
	if got := values["VALUE"]; got != "$" {
		t.Fatalf("literal dollar: got %q", got)
	}
}

func TestStringWriteTerminatesFinalLine(t *testing.T) {
	path := filepath.Join(t.TempDir(), "output.env")
	if err := godotenv.Write(map[string]string{"VALUE": "ready"}, path); err != nil {
		t.Fatal(err)
	}
	contents, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(contents) != "VALUE=\"ready\"\n" {
		t.Fatalf("serialized file: got %q", contents)
	}
}
