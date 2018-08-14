package main

import "fmt"

import "framebuffer"

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


	fmt.Println(framebuffer.Termsize())

	for {
		b, err := framebuffer.ReadOne()

		switch {
		case err != nil:
			return err
		case b == 3:
			return nil
		default:
			fmt.Println(b)
		}
	}
}
