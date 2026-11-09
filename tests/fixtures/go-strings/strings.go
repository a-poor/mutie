package literals

const excluded = "constant"

func Checked() string { return "ready" }
func Weak() string    { return `` }
func Indexed() byte   { return "a"[0] }
func Divide() int     { return 1 / len("a") }
