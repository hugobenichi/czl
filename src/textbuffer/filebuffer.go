package textbuffer

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

type Line struct {
	prev          *Line
	next          *Line
	textpieces    [][]byte
	piece_offsets []int
	len           int
}

func (line *Line) init(piece []byte) {
	assert(line.textpieces == nil)
	assert(line.piece_offsets == nil)

	line.textpieces = [][]byte{piece}
	line.piece_offsets = []int{1}
	line.len = -1
}

func (line *Line) pieces() [][]byte {
	npiece := len(line.piece_offsets)
	assert(npiece > 0)

	stop := line.piece_offsets[npiece-1]
	start := 0
	if npiece > 1 {
		start = line.piece_offsets[npiece-2]
	}

	return line.textpieces[start:stop]
}

func (line *Line) Len() int {
	if line.piece_offsets == nil {
		return 0
	}

	if line.len < 0 {
		line.len = 0
		for _, piece := range line.pieces() {
			line.len += utf8.RuneCount(piece)
		}
	}

	return line.len
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

func assert(b bool) {
	if !b {
		panic(errors.New("failed assert"))
	}
}
