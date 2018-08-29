package core

type Rec struct {
	Min Vec
	Max Vec
}

func (r Rec) X0() int   { return r.Min.X }
func (r Rec) X1() int   { return r.Max.X }
func (r Rec) Y0() int   { return r.Min.Y }
func (r Rec) Y1() int   { return r.Max.Y }
func (r Rec) W() int    { return r.Max.Y - r.Min.Y }
func (r Rec) H() int    { return r.Max.X - r.Min.X }
func (r Rec) Area() int { return r.W() * r.H() }
func (r Rec) Size() Vec { return V(r.W(), r.H()) }

func R(x0, y0, x1, y1 int) Rec {
	return Rec{V(x0, y0), V(x1, y1)}
}

func (r Rec) Translate(v Vec) Rec {
	min := v.Add(r.Min)
	max := v.Add(r.Max)
	return Rec{min, max}
}

func (r Rec) Hsplit(x int) (Rec, Rec) {
	assert(r.Min.X <= x)
	assert(x < r.Max.X)

	left := R(r.Min.X, r.Min.Y, x, r.Max.Y)
	right := R(x, r.Min.Y, r.Max.X, r.Max.Y)
	return left, right
}

func (r Rec) Vsplit(y int) (Rec, Rec) {
	assert(r.Min.Y <= y)
	assert(y < r.Max.Y)

	up := R(r.Min.X, r.Min.Y, r.Max.X, y)
	down := R(r.Min.X, y, r.Max.X, r.Max.Y)
	return up, down
}
