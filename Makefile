OUTDIR=build

.DEFAULT_GOAL := build

builddir:
	mkdir -p $(OUTDIR)

#$(OUTDIR)/czl: *.c
#	gcc -Wall -std=c99 -g -Os -o $@ $<

$(OUTDIR)/czl: czl.rs
	rustc -g --out-dir $(OUTDIR) $<

build: builddir $(OUTDIR)/czl

run: build
	$(OUTDIR)/czl

clean:
	rm -rf $(OUTDIR)
