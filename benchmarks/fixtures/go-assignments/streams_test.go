package godotenv

import (
	"io"
	"os"
	"path/filepath"
	"testing"
)

// Exercise Exec's public stream contract in a child, without invoking a shell
// or relying on the terminal running the tests. Do not run this test in parallel:
// Exec intentionally reads the process-global standard streams.
func TestAssignmentExecStreams(t *testing.T) {
	if os.Getenv("MUTIE_ASSIGNMENT_STREAM_CHILD") == "1" {
		body, err := io.ReadAll(os.Stdin)
		if err != nil {
			os.Exit(7)
		}
		_, _ = os.Stdout.Write(append([]byte("out:"), body...))
		_, _ = os.Stderr.Write([]byte("err:connected"))
		os.Exit(0)
	}
	t.Setenv("MUTIE_ASSIGNMENT_STREAM_CHILD", "1")
	dir := t.TempDir()
	envFile := filepath.Join(dir, "empty.env")
	if err := os.WriteFile(envFile, nil, 0600); err != nil {
		t.Fatal(err)
	}
	input := filepath.Join(dir, "input")
	if err := os.WriteFile(input, []byte("forwarded input"), 0600); err != nil {
		t.Fatal(err)
	}
	in, err := os.Open(input)
	if err != nil {
		t.Fatal(err)
	}
	defer in.Close()
	out, err := os.Create(filepath.Join(dir, "stdout"))
	if err != nil {
		t.Fatal(err)
	}
	defer out.Close()
	errout, err := os.Create(filepath.Join(dir, "stderr"))
	if err != nil {
		t.Fatal(err)
	}
	defer errout.Close()
	func() {
		oldIn, oldOut, oldErr := os.Stdin, os.Stdout, os.Stderr
		defer func() { os.Stdin, os.Stdout, os.Stderr = oldIn, oldOut, oldErr }()
		os.Stdin, os.Stdout, os.Stderr = in, out, errout
		err = Exec([]string{envFile}, os.Args[0], []string{"-test.run=^TestAssignmentExecStreams$"}, false)
	}()
	if err != nil {
		t.Fatal(err)
	}
	for _, check := range []struct{ path, want string }{
		{out.Name(), "out:forwarded input"},
		{errout.Name(), "err:connected"},
	} {
		got, err := os.ReadFile(check.path)
		if err != nil || string(got) != check.want {
			t.Errorf("stream %s: got %q (%v), want %q", filepath.Base(check.path), got, err, check.want)
		}
	}
}
