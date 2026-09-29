package fixture

import (
	"os"
	"testing"
	"time"
)

func TestAdd(t *testing.T) {
	if Add(2, 3) != 5 {
		t.Fatal("wrong sum")
	}
}

func TestGreeting(t *testing.T) {
	if Greeting("world") != "Hello, world" {
		t.Fatal("wrong greeting")
	}
}

func TestReady(t *testing.T) {
	for !Ready() {
		time.Sleep(time.Second)
	}
}

func TestWorkspaceIsolation(t *testing.T) {
	if _, err := os.Stat("test-artifact"); !os.IsNotExist(err) {
		t.Fatal("an earlier test run left an artifact")
	}
	state, err := os.ReadFile("testdata/state.txt")
	if err != nil || string(state) != "pristine\n" {
		t.Fatal("an earlier test run changed a file")
	}
	if err := os.WriteFile("test-artifact", []byte("created"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile("testdata/state.txt", []byte("changed"), 0600); err != nil {
		t.Fatal(err)
	}
}
