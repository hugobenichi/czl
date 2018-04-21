OUTDIR=build

.DEFAULT_GOAL := build

builddir:
	mkdir -p $(OUTDIR)

$(OUTDIR)/czl: *.c
	gcc -std=c99 -Os -o $@ $<

build: builddir $(OUTDIR)/czl

run: build
	$(OUTDIR)/czl

clean:
	rm -rf $(OUTDIR)
