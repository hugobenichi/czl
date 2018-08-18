package main

import (
	"fmt"
	"os"
	"runtime"

	"framebuffer"
	tb "textbuffer"
)

func main() {
	fmt.Println(framebuffer.Termsize())

	err := main_loop()
	if err != nil {
		panic(err)
	}
}

func main_loop() error {
	restore, err := framebuffer.Term_setraw()
	if err != nil {
		panic(err)
	}
	defer restore()

	// Clear screen
	os.Stdout.WriteString("\x1bc")
	os.Stdout.Sync()

	// Load File
	_, filename, _, _ := runtime.Caller(1)
	filebuffer, err := tb.Load(filename)
	if err != nil {
		return err
	}
	fmt.Printf("file %v length: %v lines", filebuffer.Filename, filebuffer.Nline)

	ch := framebuffer.GetInputChannel()

	for {
		input := <-ch

		switch {
		case input.Kind == framebuffer.Error:
			return input.Err
		case input.Char == framebuffer.CTRL_C:
			return nil
		case input.Kind == framebuffer.Char:
			fmt.Println(input.Char)
		case input.Kind == framebuffer.MouseClick:
			fmt.Println("mouse click")
		case input.Kind == framebuffer.MouseRelease:
			fmt.Println("mouse release")
		case input.Kind == framebuffer.Resize:
			x, y := framebuffer.Termsize()
			fmt.Println("resize: ", x, y)
		default:
			fmt.Println("unrecognized input", input)
		}
	}
}
