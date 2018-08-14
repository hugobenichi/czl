package framebuffer

import "golang.org/x/crypto/ssh/terminal"
import "golang.org/x/sys/unix"
//import "syscall"

// #include <termios.h>
// #include <sys/ioctl.h>
//import "C"
//import "unsafe"

type Winsize struct {
	Row			uint16
	Column	uint16
	XPixel	uint16
	YPixel	uint16
}

//func Termsize() Winsize {
//	w := Winsize {}
//	ptr := uintptr(unsafe.Pointer(value))
//	C.ioctl(1, C.TIOCGWINSZ, ptr)
//	return w //Winsize { 0, 0 }
//}
//

func die_if(err error) {
	if (err != nil) {
		panic(err)
	}
}

func Termsize() (int, int) {
	w, h, err := terminal.GetSize(1)
	die_if(err)
	return w, h
}

const (
	STDIN_FD = 0
	STDOUT_FD = 1
	STDERR_FD = 2
)

func Term_setraw() (func(), error) {
	original_term, err := unix.IoctlGetTermios(STDIN_FD, unix.TCGETS)
	if err != nil {
		return nil, err
	}

	raw_term := *original_term
	// This attempts to replicate the behaviour documented for cfmakeraw in
	// the termios(3) manpage.
	raw_term.Iflag &^= unix.IGNBRK | unix.BRKINT | unix.PARMRK | unix.ISTRIP | unix.INLCR | unix.IGNCR | unix.ICRNL | unix.IXON
	raw_term.Oflag &^= unix.OPOST
	raw_term.Lflag &^= unix.ECHO | unix.ECHONL | unix.ICANON | unix.ISIG | unix.IEXTEN
	raw_term.Cflag &^= unix.CSIZE | unix.PARENB
	raw_term.Cflag |= unix.CS8
	raw_term.Cc[unix.VMIN] = 1
	raw_term.Cc[unix.VTIME] = 0
	// TODO: tweak values

	if err := unix.IoctlSetTermios(STDIN_FD, unix.TCSETS, &raw_term); err != nil {
		return nil, err
	}

	return func() {
		// ignore errors: too late anyway
		unix.IoctlSetTermios(STDIN_FD, unix.TCSETS, original_term)
	}, nil
}
