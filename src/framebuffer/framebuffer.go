package framebuffer

import "golang.org/x/crypto/ssh/terminal"
import "golang.org/x/sys/unix"
import "os"
import "errors"
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
	w, h, err := terminal.GetSize(STDIN_FD)
	die_if(err)
	return w, h
}

const (
	STDIN_FD = 0
	STDOUT_FD = 1
	STDERR_FD = 2
)

func Term_setraw() (func(), error) {
	original, err := unix.IoctlGetTermios(STDIN_FD, unix.TCGETS)
	if err != nil {
		return nil, err
	}

	// TODO: check errors
	os.Stdout.WriteString("\x1b[s")            // save cursor
	os.Stdout.WriteString("\x1b[?47h")         // go offscreen
	os.Stdout.WriteString("\x1b[?1000h")       // get mouse event
	os.Stdout.WriteString("\x1b[?1002h")       // track mouse event
	os.Stdout.WriteString("\x1b[?1004h")       // get focus event
	os.Stdout.Sync()

	raw_term := *original
	// replicate behaviour documented for cfmakeraw in termios(3) manpage.
	raw_term.Iflag &^= unix.IGNBRK			// ???
	raw_term.Iflag &^= unix.BRKINT		 	// no break
	raw_term.Iflag &^= unix.PARMRK		 	// ???
	raw_term.Iflag &^= unix.ISTRIP		 	// no strip character
	raw_term.Iflag &^= unix.INLCR				// ???
	raw_term.Iflag &^= unix.INPCK				// no parity check
	raw_term.Iflag &^= unix.IGNCR		 		// no break
	raw_term.Iflag &^= unix.ICRNL		 		// no CR to NL conversion
	raw_term.Iflag &^= unix.IXON		 		// ???
	raw_term.Oflag &^= unix.OPOST				// No post processing
	raw_term.Lflag &^= unix.ECHO			  // No echo
	raw_term.Lflag &^= unix.ECHONL
	raw_term.Lflag &^= unix.ICANON
	raw_term.Lflag &^= unix.ISIG
	raw_term.Lflag &^= unix.IEXTEN
	raw_term.Cflag &^= unix.CSIZE
	raw_term.Cflag &^= unix.PARENB
	raw_term.Cflag |= unix.CS8					// 8 bits chars
	raw_term.Cc[unix.VMIN] = 0				  // return each byte, or nothing when timeout
	raw_term.Cc[unix.VTIME] = 100			  // 100 * 100 ms timeout

	if err := unix.IoctlSetTermios(STDIN_FD, unix.TCSETS, &raw_term); err != nil {
		return nil, err
	}

	return func() {
		os.Stdout.WriteString("\x1b[?1004l")   // stop focus event
		os.Stdout.WriteString("\x1b[?1002l")   // stop mouse tracking
		os.Stdout.WriteString("\x1b[?1000l")   // stop mouse event
		os.Stdout.WriteString("\x1b[?47l")     // go back to main screen
		os.Stdout.WriteString("\x1b[u")        // restore cursor
		os.Stdout.Sync()

		// ignore errors: too late anyway
		unix.IoctlSetTermios(STDIN_FD, unix.TCSETS, original)
	}, nil
}

func ReadOne() (byte, error) {
	buffer := make([]byte, 1)
	n, err := os.Stdout.Read(buffer)
	switch {
	case err != nil:
		return 0, err
	case n == 0:
		return 0, errors.New("timeout")
	case n > 1:
		return 0, errors.New("wtf")
	default: // n == 1
		return buffer[0], nil
	}
}
