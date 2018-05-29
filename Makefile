OUTDIR=build

.DEFAULT_GOAL := build

builddir:
	mkdir -p $(OUTDIR)

# ar -r cs archive files on osx but ar -rcs archive files on linux ???
$(OUTDIR)/libterm.a: builddir term.c
	gcc -c term.c -o $(OUTDIR)/term.o
	ar -r cs $(OUTDIR)/libterm.a $(OUTDIR)/term.o
	#ar -rcs $(OUTDIR)/libterm.a $(OUTDIR)/term.o
	#gcc -shared -Wl -o $(OUTDIR)/libterm.a $(OUTDIR)/term.o -lc

native: builddir $(OUTDIR)/libterm.a

$(OUTDIR)/czl: czl.rs
	rustc -g --out-dir $(OUTDIR) -L ./$(OUTDIR) $<

build: builddir native $(OUTDIR)/czl

run: build
	$(OUTDIR)/czl

clean:
	rm -rf $(OUTDIR)
