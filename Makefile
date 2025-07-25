NAME = zoi

COMMIT_HASH := $(shell git rev-parse --short=10 HEAD)

ifeq ($(OS),Windows_NT)
    IS_WINDOWS := 1
    SRC_BIN = target/release/$(NAME).exe
else
    IS_WINDOWS := 0
    SRC_BIN = target/release/$(NAME)
endif

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
	@if not exist "$(USERPROFILE)\Documents\PowerShell" mkdir "$(USERPROFILE)\Documents\PowerShell"
	@target\release\$(NAME).exe generate-completions powershell >> "$(USERPROFILE)\Documents\PowerShell\Microsoft.PowerShell_profile.ps1"
	@echo ""
	@echo "Completion script appended to your PowerShell profile."
	@echo "Please restart your shell or run '. $PROFILE' to activate it."
else
	@echo "  -> Bash"
	@mkdir -p ~/.local/share/bash-completion/completions
	@./target/release/$(NAME) generate-completions bash > ~/.local/share/bash-completion/completions/$(NAME)

	@echo "  -> Zsh"
	@mkdir -p ~/.zsh/completions
	@./target/release/$(NAME) generate-completions zsh > ~/.zsh/completions/_$(NAME)

	@echo "  -> Fish"
	@mkdir -p ~/.config/fish/completions
	@./target/release/$(NAME) generate-completions fish > ~/.config/fish/completions/$(NAME).fish

	@echo "  -> Elvish"
	@mkdir -p ~/.config/elvish/completions
	@./target/release/$(NAME) generate-completions elvish > ~/.config/elvish/completions/$(NAME).elv
	
	@echo ""
	@echo "Completion scripts installed."
	@echo "Please restart your shell or source your shell's profile to activate them."
endif
