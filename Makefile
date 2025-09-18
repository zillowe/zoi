NAME = zoi

COMMIT_HASH := $(shell git rev-parse --short=10 HEAD)

IS_WINDOWS := 0
SRC_BIN = target/release/$(NAME)

ifneq (,$(wildcard config.mk))
    include config.mk
else
    $(error config.mk not found. Please run ./configure first.)
endif

ifeq ($(OS_NAME),windows)
    IS_WINDOWS := 1
    SRC_BIN = target/release/$(NAME).exe
endif

.PHONY: all build install uninstall clean setup help

all: build install setup
	@echo "Done"

build: $(SRC_BIN)
	@echo "Building Zoi in release mode (commit: $(COMMIT_HASH))..."
	@ZOI_COMMIT_HASH=$(COMMIT_HASH) cargo build --bin zoi --release
	@echo "Build complete for $(OS_NAME) ($(ARCH_NAME))."

install:
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

setup:
	@echo "Running setup for the '$(SHELL_NAME)' shell..."
	@$(SRC_BIN) shell $(SHELL_NAME)
	@$(SRC_BIN) setup
	@echo ""
	@echo "Setup complete."
	@echo "Please restart your shell or source your shell's profile to apply changes."

help:
	@echo "make 		alias to 'make all'"
	@echo "make build 	build zoi in release mode"
	@echo "make install 	install Zoi binary to PREFIX or default user's bin location"
	@echo "make setup 	install shell completion and setup Zoi's package PATH"
	@echo "make uninstall 	uninstall Zoi binary"
	@echo "make clean 	clean project artifacts"
	@echo "make all 	run 'make', 'make install' and 'make setup'"
