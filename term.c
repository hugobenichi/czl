#include <termios.h>
#include <errno.h>
#include <unistd.h>

#include <sys/ioctl.h>


struct winsize terminal_get_size()
{
	struct winsize w = {};
	ioctl(1, TIOCGWINSZ, &w);
	return w;
}


struct termios termios_initial = {};


void terminal_restore()
{
	tcsetattr(STDIN_FILENO, TCSAFLUSH, &termios_initial);
}


int terminal_set_raw()
{
	int z;
	z = tcgetattr(STDIN_FILENO, &termios_initial);
	if (z < 0) {
		return z;
	}

	struct termios termios_raw = termios_initial;
	termios_raw.c_iflag &= ~BRKINT;                    // no break
	termios_raw.c_iflag &= ~ICRNL;                     // no CR to NL
	termios_raw.c_iflag &= ~INPCK;                     // no parity check
	termios_raw.c_iflag &= ~ISTRIP;                    // no strip character
	termios_raw.c_iflag &= ~IXON;                      // no CR to NL
	termios_raw.c_oflag &= ~OPOST;                     // no post processing
	termios_raw.c_lflag &= ~ECHO;                      // no echo
	termios_raw.c_lflag &= ~ICANON;
	termios_raw.c_lflag &= ~ISIG;
	termios_raw.c_cc[VMIN] = 0;                        // return each byte, or nothing when timeout
	termios_raw.c_cc[VTIME] = 100;                     // 100 * 100 ms timeout
	termios_raw.c_cflag |= CS8;                        // 8 bits chars

	return tcsetattr(STDIN_FILENO, TCSAFLUSH, &termios_raw);
}

static const int RESIZE = 0xff;

int read_1B()
{
	char c;

	int n = 0;
	while (n == 0) {
		n = read(STDIN_FILENO, &c, 1);
	}

	// Terminal resize events send SIGWINCH signals which interrupt read()
	if (n < 0 && errno == EINTR) {
		return RESIZE;
	}

	if (n < 0) {
		return -errno;
	}

	return c;
}
