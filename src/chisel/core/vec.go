package core

type Vec struct {
	X int
	Y int
}

func V(x, y int) Vec {
	return Vec{x, y}
}

func (v Vec) Add(w Vec) Vec {
	x := v.X + w.X
	y := v.Y + w.Y
	return V(x, y)
}

func (v Vec) Sub(w Vec) Vec {
	x := v.X - w.X
	y := v.Y - w.Y
	return V(x, y)
}

func (v Vec) Minus() Vec {
	return V(-v.X, -v.Y)
}

func (v Vec) Translate(r Rec) Rec {
	min := v.Add(r.Min)
	max := v.Add(r.Max)
	return Rec{min, max}
}
