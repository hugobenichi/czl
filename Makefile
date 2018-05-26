OUTDIR=build

.DEFAULT_GOAL := buildrs

builddir:
	mkdir -p $(OUTDIR)

$(OUTDIR)/czl: *.c
	gcc -Wall -std=c99 -g -Os -o $@ $<

$(OUTDIR)/term: term.rs
	rustc -g --out-dir $(OUTDIR) $<

buildrs: builddir $(OUTDIR)/term

build: builddir $(OUTDIR)/czl

run: build
	$(OUTDIR)/czl

runrs: buildrs
	$(OUTDIR)/term

clean:
	rm -rf $(OUTDIR)
