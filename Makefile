NAME = zoi

COMMIT_HASH := $(shell git rev-parse --short=10 HEAD)

IS_WINDOWS := 0
SRC_BIN = target/release/$(NAME)

ifeq ($(OS_NAME),windows)
    IS_WINDOWS := 1
    SRC_BIN = target/release/$(NAME).exe
endif

ifneq (,$(wildcard config.mk))
    include config.mk
else
    $(error config.mk not found. Please run ./configure first.)
endif

.PHONY: all install uninstall clean install-completions

all: $(SRC_BIN)
	@echo "Build complete for $(OS_NAME) ($(ARCH_NAME))."

$(SRC_BIN):
	@echo "Building Zoi in release mode (commit: $(COMMIT_HASH))..."
	@ZOI_COMMIT_HASH=$(COMMIT_HASH) cargo build --release

install: all
ifeq ($(IS_WINDOWS),1)
	@echo "Installing $(NAME) to $(BINDIR)..."
	@if not exist "$(BINDIR)" mkdir "$(BINDIR)"
	@copy /Y "$(SRC_BIN)" "$(BINDIR)\$(NAME).exe"
	@echo "Zoi installed successfully to $(BINDIR)\$(NAME).exe"
	@echo "Make sure '$(BINDIR)' is in your system's PATH."
else
	@echo "Installing $(NAME) to $(BINDIR)..."
	@mkdir -p "$(BINDIR)"
	@install -m 755 "$(SRC_BIN)" "$(BINDIR)/$(NAME)"
	@echo "Zoi installed successfully to $(BINDIR)/$(NAME)"
	@echo "Make sure '$(BINDIR)' is in your shell's PATH."
endif

uninstall:
ifeq ($(IS_WINDOWS),1)
	@echo "Uninstalling $(NAME) from $(BINDIR)..."
	@if exist "$(BINDIR)\$(NAME).exe" del /f "$(BINDIR)\$(NAME).exe"
	@echo "Zoi uninstalled."
else
	@echo "Uninstalling $(NAME) from $(BINDIR)..."
	@rm -f "$(BINDIR)/$(NAME)"
	@echo "Zoi uninstalled."
endif

clean:
	@echo "Cleaning project artifacts..."
	@cargo clean
ifeq ($(IS_WINDOWS),1)
	@if exist config.mk del config.mk
else
	@rm -f config.mk
endif

install-completions: all
	@echo "Installing shell completions..."
ifeq ($(IS_WINDOWS),1)
	@echo "  -> PowerShell"
	@./target/release/$(NAME).exe shell powershell
else
	@echo "  -> Bash"
	@./target/release/$(NAME) shell bash
	@echo "  -> Zsh"
	@./target/release/$(NAME) shell zsh
	@echo "  -> Fish"
	@./target/release/$(NAME) shell fish
	@echo "  -> Elvish"
	@./target/release/$(NAME) shell elvish
endif
	@echo ""
	@echo "Completion scripts installed."
	@echo "Please restart your shell or source your shell's profile to activate them."
