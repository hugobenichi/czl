#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <termios.h>
#include <unistd.h>

#include <sys/ioctl.h>


static const char terminal_cursor_save[]                  = "\x1b[s";
static const char terminal_cursor_restore[]               = "\x1b[u";
static const char terminal_switch_offscreen[]             = "\x1b[?47h";
static const char terminal_switch_mainscreen[]            = "\x1b[?47l";
static const char terminal_switch_mouse_event_on[]        = "\x1b[?1000h";
static const char terminal_switch_mouse_tracking_on[]     = "\x1b[?1002h";
static const char terminal_switch_mouse_tracking_off[]    = "\x1b[?1002l";
static const char terminal_switch_mouse_event_off[]       = "\x1b[?1000l";
static const char terminal_switch_focus_event_on[]        = "\x1b[?1004h";
static const char terminal_switch_focus_event_off[]       = "\x1b[?1004l";


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
	//fprintf(stdout, terminal_switch_focus_event_off);
	fprintf(stdout, terminal_switch_mouse_tracking_off);
	fprintf(stdout, terminal_switch_mouse_event_off);
	fprintf(stdout, terminal_switch_mainscreen);
	fprintf(stdout, terminal_cursor_restore);
}


int terminal_set_raw()
{
	int z;
	z = tcgetattr(STDIN_FILENO, &termios_initial);
	if (z < 0) {
		perror("terminal_raw: tcgetattr() failed");
		return z;
	}
	atexit(terminal_restore);

	fprintf(stdout, terminal_cursor_save);
	fprintf(stdout, terminal_switch_offscreen);
	fprintf(stdout, terminal_switch_mouse_event_on);
	fprintf(stdout, terminal_switch_mouse_tracking_on);
	//fprintf(stdout, terminal_switch_focus_event_on); // TODO: turn on and check if files need reload.

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

	z = tcsetattr(STDIN_FILENO, TCSAFLUSH, &termios_raw);
	if (z < 0) {
		errno = ENOTTY;
		perror("terminal_set_raw: tcsetattr() failed");
	}
	return z;
}
