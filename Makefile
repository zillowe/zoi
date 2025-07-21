NAME = zoi

COMMIT_HASH := $(shell git rev-parse --short=10 HEAD)

SRC_BIN = target/release/$(NAME)

ifneq (,$(wildcard config.mk))
    include config.mk
else
    $(error config.mk not found. Please run ./configure first.)
endif

.PHONY: all install uninstall clean install-completions

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

install-completions: all
	@echo "Installing shell completions..."
	
	@echo "  -> Bash"
	@mkdir -p ~/.local/share/bash-completion/completions
	@./target/release/$(NAME) generate-completions bash > ~/.local/share/bash-completion/completions/$(NAME)

	@echo "  -> Zsh"
	@mkdir -p ~/.zsh/completions
	@./target/release/$(NAME) generate-completions zsh > ~/.zsh/completions/_$(NAME)

	@echo "  -> Fish"
	@mkdir -p ~/.config/fish/completions
	@./target/release/$(NAME) generate-completions fish > ~/.config/fish/completions/$(NAME).fish
	
	@echo ""
	@echo "Completion scripts installed."
	@echo "Please restart your shell or source your shell's profile to activate them."
