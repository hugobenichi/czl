OUTDIR=build

.DEFAULT_GOAL := build

builddir:
	mkdir -p $(OUTDIR)

$(OUTDIR)/term.o: builddir term.c
	gcc -c term.c -o $(OUTDIR)/term.o

termlinux: builddir $(OUTDIR)/term.o
	ar -rcs $(OUTDIR)/libterm.a $(OUTDIR)/term.o

termosx: builddir $(OUTDIR)/term.o
	ar rcs $(OUTDIR)/libterm.a $(OUTDIR)/term.o

native: builddir $(OUTDIR)/libterm.a

$(OUTDIR)/czl: czl.rs
	rustc -C opt-level=1 -g --out-dir $(OUTDIR) -L ./$(OUTDIR) $<

build: builddir native $(OUTDIR)/czl

run: build
	env RUST_BACKTRACE=1 $(OUTDIR)/czl

clean:
	rm -rf $(OUTDIR)

gobuild:
	#go build vec.go rec.go
	GOBIN=${PWD}/build/ go install chi

gorun: gobuild
	./build/main
