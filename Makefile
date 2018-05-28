OUTDIR=build

.DEFAULT_GOAL := build

builddir:
	mkdir -p $(OUTDIR)

#$(OUTDIR)/czl: *.c
#	gcc -Wall -std=c99 -g -Os -o $@ $<

$(OUTDIR)/libterm.a: builddir term.c
	gcc -c term.c -o $(OUTDIR)/term.o
	gcc -shared -Wl -o $(OUTDIR)/libterm.a $(OUTDIR)/term.o -lc

native: builddir $(OUTDIR)/libterm.a

$(OUTDIR)/czl: czl.rs
	rustc -g --out-dir $(OUTDIR) -L ./$(OUTDIR) $<

build: builddir native $(OUTDIR)/czl

run: build
	$(OUTDIR)/czl

clean:
	rm -rf $(OUTDIR)
