OUTDIR=build

.DEFAULT_GOAL := build

builddir:
	mkdir -p $(OUTDIR)

$(OUTDIR)/czl: *.c
	gcc -Wall -std=c99 -g -Os -o $@ $<

build: builddir $(OUTDIR)/czl

run: build
	$(OUTDIR)/czl

clean:
	rm -rf $(OUTDIR)
