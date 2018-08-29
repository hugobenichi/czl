package main

import (
	"fmt"
	"os"
	"runtime"

	chisel "chisel/core"
)

func main() {
	chisel.InitCommands()

	fmt.Println(chisel.Termsize())

	err := main_loop()
	if err != nil {
		panic(err)
	}
}

func main_loop() error {
	restore, err := chisel.Term_setraw()
	if err != nil {
		panic(err)
	}
	defer restore()

	// Clear screen
	os.Stdout.WriteString("\x1bc")
	os.Stdout.Sync()

	// Load File
	_, filename, _, _ := runtime.Caller(1)
	filebuffer, err := chisel.Load(filename)
	if err != nil {
		return err
	}
	fmt.Printf("file %v length: %v lines", filebuffer.Filename, filebuffer.Nline)

	ch := chisel.GetInputChannel()

	for {
		input := <-ch

		switch {
		case input.Kind == chisel.Error:
			return input.Err
		case input.Char == chisel.CTRL_C:
			return nil
		case input.Kind == chisel.Char:
			fmt.Println(input.Char)
		case input.Kind == chisel.MouseClick:
			fmt.Println("mouse click")
		case input.Kind == chisel.MouseRelease:
			fmt.Println("mouse release")
		case input.Kind == chisel.Resize:
			x, y := chisel.Termsize()
			fmt.Println("resize: ", x, y)
		default:
			fmt.Println("unrecognized input", input)
		}
	}
}
