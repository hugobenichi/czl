OUTDIR=build

.DEFAULT_GOAL := build

builddir:
	mkdir -p $(OUTDIR)

$(OUTDIR)/libterm.a: builddir term.c
	gcc -c term.c -o $(OUTDIR)/term.o
	#ar -r cs $(OUTDIR)/libterm.a $(OUTDIR)/term.o # works on both linux and osx

native: builddir $(OUTDIR)/libterm.a

$(OUTDIR)/czl: czl.rs
	rustc -g --out-dir $(OUTDIR) -L ./$(OUTDIR) $<

build: builddir native $(OUTDIR)/czl

run: build
	$(OUTDIR)/czl

clean:
	rm -rf $(OUTDIR)
