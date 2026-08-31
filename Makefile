PREFIX ?= $(HOME)/.local
BIN ?= $(PREFIX)/bin
MANDIR ?= $(PREFIX)/share/man/man1
CLI = cli/copypaste.mjs

.PHONY: cli test test-cli install serve spec doctor

cli:
	chmod +x $(CLI)
	$(CLI) version

test: test-cli
	node --experimental-strip-types --test 'src/lib/**/*.test.ts'

test-cli: cli
	node --test cli/copypaste.test.mjs

install: cli
	mkdir -p $(BIN) $(MANDIR)
	cp $(CLI) $(BIN)/copypaste
	chmod 0755 $(BIN)/copypaste
	cp packaging/man/copypaste.1 $(MANDIR)/copypaste.1
	$(BIN)/copypaste version

serve:
	$(CLI) serve --port 8787 --bind 127.0.0.1

spec:
	$(CLI) spec

doctor:
	$(CLI) doctor
