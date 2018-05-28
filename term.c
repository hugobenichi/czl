#include <stdint.h>
#include <sys/ioctl.h>

struct winsize get_terminal_size() {
  struct winsize w = {};
  ioctl(1, TIOCGWINSZ, &w);
  return w;
}
