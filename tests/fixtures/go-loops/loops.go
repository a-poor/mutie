package loops

import "strings"

func Count(n int) int {
	total := 0
	for i := 0; i < n; i++ {
		total++
	}
	return total
}

func Local(b bool) int {
	local := b
	for local {
		return 1
	}
	return 0
}

// The import is used only by the condition.
func ImportOnly() int {
	for strings.Contains("abc", "b") {
		return 1
	}
	return 0
}

type Flag bool

func Named(b Flag) int {
	for b {
		return 1
	}
	return 0
}

func Generic[T ~bool](b T) int {
	for b {
		return 1
	}
	return 0
}

// A mutation guard must not depend on the shadowable false identifier.
func Shadowed(true, false bool) int {
	for true != false {
		return 1
	}
	return 0
}

func Effects() (initializations, conditions, bodies, updates int) {
	initialize := func() { initializations++ }
	condition := func() bool { conditions++; return bodies < 2 }
	update := func() { updates++ }
	for initialize(); condition(); update() {
		bodies++
	}
	return
}

// This mutant survives; preserving the initializer is still observable.
func InitOnly() int {
	calls := 0
	initialize := func() bool { calls++; return false }
	for b := initialize(); b; {
		panic("unreachable body")
	}
	return calls
}

func Unchecked(n int) int {
	total := 0
	for ; n > 0; n-- {
		total++
	}
	return total
}
