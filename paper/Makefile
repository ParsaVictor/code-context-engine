# Build the paper. Needs a TeX distribution (pdflatex + bibtex), or tectonic (`make tectonic`).
# `make check` runs the static checks only (Python 3, no TeX needed).

TEX     ?= pdflatex
BIBTEX  ?= bibtex
PYTHON  ?= python3
FLAGS   = -interaction=nonstopmode -halt-on-error
SOURCES = main.tex refs.bib $(wildcard tables/*.tex)

.PHONY: all twocol tectonic check todo arxiv clean

all: main.pdf

main.pdf: $(SOURCES)
	$(TEX) $(FLAGS) main.tex
	$(BIBTEX) main
	$(TEX) $(FLAGS) main.tex
	$(TEX) $(FLAGS) main.tex

# Two-column layout from the same source.
twocol: main-2col.pdf

main-2col.pdf: $(SOURCES)
	$(TEX) $(FLAGS) -jobname=main-2col "\def\twocolumnmode{1}\input{main.tex}"
	$(BIBTEX) main-2col
	$(TEX) $(FLAGS) -jobname=main-2col "\def\twocolumnmode{1}\input{main.tex}"
	$(TEX) $(FLAGS) -jobname=main-2col "\def\twocolumnmode{1}\input{main.tex}"

tectonic: $(SOURCES)
	tectonic main.tex

check:
	$(PYTHON) check.py

# Fails while any \todo remains (use before submission).
todo:
	@! grep -n '\\todo{' main.tex tables/*.tex

# arXiv wants the .bbl, not the .bib. Builds first, then packs the sources.
arxiv: main.pdf
	tar czf arxiv.tar.gz main.tex main.bbl tables/*.tex

clean:
	rm -f *.aux *.bbl *.blg *.log *.out *.toc *.pdf arxiv.tar.gz
