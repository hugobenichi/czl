package czl

type Rec struct {
	Min Vec
	Max Vec
}

func R(min, max Vec) Rec {
	return Rec { min, max }
}

func (r Rec) Translate(v Vec) Rec {
	min := v.Add(r.Min)
	max := v.Add(r.Max)
	return R(min, max)
}
