package command

type LineCommand int

const (
		LineNoop LineCommand = iota // nothing
		LineInsert // insert a new line at cursor position, shifting current line down
		LineAppend // insert a new line after cursor position, shifting current line up
		LineDel    // delete current cursor lines
		LineBreak  // break line at cursor position
		LineJoin	 // join current lines together
		// These should be object + copy
		LineCopy	 // copy current lines in buffer
		LinePaste	 // copy current lines in buffer
)


/*

language for manipulating test

object + verb -> object | action on object
object = cursor | text block
text block = line, multi line, column, multi cursor

verb = move | edit |
move = up, down, left, right, first, last

action = cursor action | text action

*/


func Init() {

	// language for manipulating text

	// TODO: make this compile
	//rule("command").is("object").and("verb").produces("object", "action")

	group("action").is("nothing")

	group("verb").is("move", "edit", "undo", "redo", "quit")
	group("move").is("up", "down", "left", "right", "first", "last")
	group("edit").is("insert", "blank", "append", "blank", "delete", "copy", "paste", "replace")

	group("object").is("cursor", "line", "range", "block", "file", "tab", "window") // THINK: how to define multiple and composite objects ?
}

type Group struct {}

type SymbolKind int

const (
	SymbolUnknown SymbolKind = iota
	SymbolAtom
	SymbolGroup
	SymbolRule
)

type Symbol struct {
	kind SymbolKind
	name string
	id int
}


var (
	symbol_table = []Symbol {}
	symbol_by_name = make(map[string]Symbol)
)

func symbol(name string) int {
	if s, defined := symbol_by_name[name]; defined {
		return s.id
	}
	s := Symbol {
		kind: SymbolUnknown,
		name: name,
		id: len(symbol_table),
	}
	symbol_table = append(symbol_table, s)
	return s.id
}

func group(name string) *Group {
	id := symbol(name)
	s := &symbol_table[id]
	if s.kind != SymbolUnknown {
		panic(name + " had already a kind")
	}
	s.kind = SymbolGroup
	// TODO: store objects somewhere ! -> directly intol their Symbol thing
	return &Group {}
}

func (g *Group) is(names... string) {
	for _, n := range names {
		// TODO store names to that group
		symbol(n)
	}
}
