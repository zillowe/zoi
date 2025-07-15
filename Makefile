NAME = zoi

COMMIT_HASH := $(shell git rev-parse --short=10 HEAD)

SRC_BIN = target/release/$(NAME)

ifneq (,$(wildcard config.mk))
    include config.mk
else
    $(error config.mk not found. Please run ./configure first.)
endif

.PHONY: all install uninstall clean

all: $(SRC_BIN)

$(SRC_BIN):
	@echo "Building Zoi in release mode (commit: $(COMMIT_HASH))..."
	@ZOI_COMMIT_HASH=$(COMMIT_HASH) cargo build --release

install: all
	@echo "Installing $(NAME) to $(BINDIR)..."
	@mkdir -p "$(BINDIR)"
	@install -m 755 "$(SRC_BIN)" "$(BINDIR)/$(NAME)"
	@echo "Zoi installed successfully to $(BINDIR)/$(NAME)"
	@echo "Make sure '$(BINDIR)' is in your shell's PATH."

uninstall:
	@echo "Uninstalling $(NAME) from $(BINDIR)..."
	@rm -f "$(BINDIR)/$(NAME)"
	@echo "Zoi uninstalled."

clean:
	@echo "Cleaning project artifacts..."
	@cargo clean
	@rm -f config.mk
