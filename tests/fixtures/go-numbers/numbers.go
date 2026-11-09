package numbers

const numerator = 10
const fixed = 9

var values = [1]int{fixed}

func Checked() uint8 { return 7 }
func Weak() float64  { return 0.0 }
func Divide() int    { return numerator / 1 }
func Indexed() int   { return values[0] }
