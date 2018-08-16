package framebuffer

import "golang.org/x/crypto/ssh/terminal"
import "golang.org/x/sys/unix"
import "os"
import "syscall"
import "os/signal"

// #include <termios.h>
// #include <sys/ioctl.h>
//import "C"
//import "unsafe"

type Winsize struct {
	Row    uint16
	Column uint16
	XPixel uint16
	YPixel uint16
}

//func Termsize() Winsize {
//	w := Winsize {}
//	ptr := uintptr(unsafe.Pointer(value))
//	C.ioctl(1, C.TIOCGWINSZ, ptr)
//	return w //Winsize { 0, 0 }
//}
//

func die_if(err error) {
	if err != nil {
		panic(err)
	}
}

func Termsize() (int, int) {
	w, h, err := terminal.GetSize(STDIN_FD)
	die_if(err)
	return w, h
}

const (
	STDIN_FD  = 0
	STDOUT_FD = 1
	STDERR_FD = 2
)

func Term_setraw() (func(), error) {
	original, err := unix.IoctlGetTermios(STDIN_FD, unix.TCGETS)
	if err != nil {
		return nil, err
	}

	// TODO: check errors
	os.Stdout.WriteString("\x1b[s")      // save cursor
	os.Stdout.WriteString("\x1b[?47h")   // go offscreen
	os.Stdout.WriteString("\x1b[?1000h") // get mouse event
	os.Stdout.WriteString("\x1b[?1002h") // track mouse event
	os.Stdout.WriteString("\x1b[?1004h") // get focus event
	os.Stdout.Sync()

	raw_term := *original
	// replicate behaviour documented for cfmakeraw in termios(3) manpage.
	// Input modes
	raw_term.Iflag &^= unix.INPCK  // no parity check
	raw_term.Iflag &^= unix.PARMRK // no parity check
	raw_term.Iflag &^= unix.ISTRIP // no character stripping to 7 bits
	raw_term.Iflag &^= unix.IGNBRK // do not ignore break conditions
	raw_term.Iflag &^= unix.BRKINT // pass break conditions as '\0'
	raw_term.Iflag &^= unix.IGNCR  // keep CR on RET key
	raw_term.Iflag &^= unix.ICRNL  // no CR to NL conversion
	raw_term.Iflag &^= unix.INLCR  // no NL to CR conversion
	raw_term.Iflag &^= unix.IXON   // no start/stop control chars on output
	raw_term.Iflag &^= unix.IXOFF  // no start/stop control chars on input
	// Output modes
	raw_term.Oflag &^= unix.OPOST // no post processing
	// Local modes
	raw_term.Lflag &^= unix.ECHO   // no echo
	raw_term.Lflag &^= unix.ECHONL // no NL echo
	raw_term.Lflag &^= unix.ICANON // no canonical mode, read input as soon as available
	raw_term.Lflag &^= unix.IEXTEN // no extension
	raw_term.Lflag &^= unix.ISIG   // turn off sigint, sigquit, sigsusp
	// Control modes
	raw_term.Cflag &^= unix.PARENB // be sure to turn off parity check
	raw_term.Cflag &^= unix.CSIZE  // clear all bit-per-char flags
	raw_term.Cflag |= unix.CS8     // 8 bits chars
	raw_term.Cc[unix.VMIN] = 0     // return each byte, or nothing when timeout
	raw_term.Cc[unix.VTIME] = 100  //100			  // 100 * 100 ms timeout

	if err := unix.IoctlSetTermios(STDIN_FD, unix.TCSETS, &raw_term); err != nil {
		return nil, err
	}

	return func() {
		os.Stdout.WriteString("\x1b[?1004l") // stop focus event
		os.Stdout.WriteString("\x1b[?1002l") // stop mouse tracking
		os.Stdout.WriteString("\x1b[?1000l") // stop mouse event
		os.Stdout.WriteString("\x1b[?47l")   // go back to main screen
		os.Stdout.WriteString("\x1b[u")      // restore cursor
		os.Stdout.Sync()

		// ignore errors: too late anyway
		unix.IoctlSetTermios(STDIN_FD, unix.TCSETS, original)
	}, nil
}

type InputKind int

type Input struct {
	Kind    InputKind
	Char    rune
	Mouse_x int
	Mouse_y int
	Err     error
}

// TODO: implement String() !

const (
	Error InputKind = iota
	Char
	MouseClick
	MouseRelease
	Timeout
	Resize
	Unknown
)

func PushResizeEvents(ch chan<- Input) {
	c := make(chan os.Signal)
	signal.Notify(c, syscall.SIGWINCH)

	for {
		<-c
		ch <- Input{Kind: Resize}
	}
}

func PushInput(ch chan<- Input) {
	buffer := make([]byte, 32)

	for {
		n, err := os.Stdout.Read(buffer)
		switch {

		case err != nil:
			// timeout actually happens here
			ch <- Input{Kind: Error, Err: err}

		case n == 0:
			// Does not happen ??
			ch <- Input{Kind: Timeout}

		case n == 1:
			// Normal output
			ch <- Input{Kind: Char, Char: rune(buffer[0])}

		case n == 3 && buffer[1] == '[' && buffer[2] == 'Z':
			// shift + tab -> "\x1b[Z" escape sequence
			ch <- Input{Kind: Char, Char: ESC_Z}

		case n >= 3 && buffer[1] == '[' && buffer[2] == 'M':
			// TODO: parse mouse click
			ch <- Input{Kind: MouseClick}

		// TODO: need to distinguish unicode input below
		default:
			ch <- Input{Kind: Unknown}
		}
	}
}

func GetInputChannel() <-chan Input {
	ch := make(chan Input)

	go PushResizeEvents(ch)
	go PushInput(ch)

	return ch
}

var (
	CTRL_AT            = '\x00'
	CTRL_A             = '\x01'
	CTRL_B             = '\x02'
	CTRL_C             = '\x03'
	CTRL_D             = '\x04'
	CTRL_E             = '\x05'
	CTRL_F             = '\x06'
	CTRL_G             = '\x07'
	CTRL_H             = '\x08'
	CTRL_I             = '\x09'
	CTRL_J             = '\x0a'
	CTRL_K             = '\x0b'
	CTRL_L             = '\x0c'
	CTRL_M             = '\x0d'
	CTRL_N             = '\x0e'
	CTRL_O             = '\x0f'
	CTRL_P             = '\x10'
	CTRL_Q             = '\x11'
	CTRL_R             = '\x12'
	CTRL_S             = '\x13'
	CTRL_T             = '\x14'
	CTRL_U             = '\x15'
	CTRL_V             = '\x16'
	CTRL_W             = '\x17'
	CTRL_X             = '\x18'
	CTRL_Y             = '\x19'
	CTRL_Z             = '\x1a'
	CTRL_LEFT_BRACKET  = '\x1b'
	CTRL_BACKSLASH     = '\x1c'
	CTRL_RIGHT_BRACKET = '\x1d'
	CTRL_CARET         = '\x1e'
	CTRL_UNDERSCORE    = '\x1f'
	SPACE              = '\x20'
	DEL                = '\x7f'
	ESC                = CTRL_LEFT_BRACKET
	BACKSPACE          = CTRL_H
	TAB                = CTRL_I
	LINE_FEED          = CTRL_J
	VTAB               = CTRL_K
	NEW_PAGE           = CTRL_L
	ENTER              = CTRL_M

	ESC_Z = rune(1000) // TODO: get a nonunicode value !
)
