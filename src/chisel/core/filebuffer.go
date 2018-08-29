package core

/*
	TODOs:
		- Load a file
		- Safe a file -> need a line Writer or line iterator
		- Implement history buffer for ops
		- Implement do, undo, redo
		- Define commands in Line mode
		- Define move mode
*/

import (
	"errors"
	"io/ioutil"
	"unicode/utf8"
)

type Filebuffer struct {
	Filename string
	Nline    int

	textbuffer []byte
	firstline  *Line
}

type Cursor struct {
	file   *Filebuffer
	line   *Line
	lineno int
	colno  int
}

func Load(filename string) (*Filebuffer, error) {
	data, err := ioutil.ReadFile(filename)
	if err != nil {
		return nil, err
	}

	nline := 0
	// Parse lines
	line := Line{}

	return &Filebuffer{
		Filename:   filename,
		textbuffer: data,
		firstline:  &line,
		Nline:      nline,
	}, nil
}

// Define ops scoped on one line, define ops scoped on multiple lines or filebuffer

type Line struct {
	prev *Line
	next *Line
	head []byte
	tail [][]byte
	len  int
}

func (line *Line) init(piece []byte) {
	assert(line.tail == nil)
	assert(line.head == nil)
	line.head = piece
	line.len = -1
}

func (line *Line) Len() int {
	if line.head == nil {
		return 0
	}

	if line.len < 0 {
		line.len = utf8.RuneCount(line.head)
		for _, piece := range line.tail {
			line.len += utf8.RuneCount(piece)
		}
	}

	return line.len
}

type Opkind int

const (
	OpLineInsert Opkind = iota
	OpLineDelete
	OpLineEdit
)

type Op struct {
	Kind   Opkind
	Upper  *Line
	Second *Line
}

func assert(b bool) {
	if !b {
		panic(errors.New("failed assert"))
	}
}
