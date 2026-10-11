package branches

import "strings"

func Choose(b bool) int {
	local := b
	if local {
		return 1
	}
	return 2
}

var Initializations int

func initialize(b bool) bool { Initializations++; return b }

func Initialized(b bool) int {
	if x := initialize(b); x {
		return 1
	}
	return 2
}

// Forcing this true must preserve the initializer's side effect.
func InitOnly() int {
	if x := initialize(true); x {
		return 1
	}
	return 2
}

// The import is used only in this condition; both mutations must still build.
func ImportOnly() int {
	if strings.Contains("abc", "b") {
		return 1
	}
	return 2
}

type Flag bool

func Named(b Flag) int {
	if b {
		return 1
	}
	return 2
}

func Generic[T ~bool](b T) int {
	if b {
		return 1
	}
	return 2
}

// true and false are legal Go parameter names. Mutation guards cannot use them.
func Shadowed(true, false bool) int {
	if true != false {
		return 1
	}
	return 2
}

var Evaluations int

func condition() bool { Evaluations++; return true }

func Effect() int {
	if condition() {
		return 1
	}
	return 2
}

func Unchecked(b bool) int {
	if b {
		return 1
	}
	return 2
}
