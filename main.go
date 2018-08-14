package main

import "fmt"

import "framebuffer"

func main() {
	fmt.Println(framebuffer.Termsize())


	restore, err := framebuffer.Term_setraw()
	if err != nil {
		panic(err)
	}
	defer restore()


	fmt.Println(framebuffer.Termsize())

	framebuffer.ReadOne()
}
