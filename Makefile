NAME = zoi
MINI_NAME = zoi-mini

COMMIT_HASH := $(shell git rev-parse --short=10 HEAD)

IS_WINDOWS := 0
SRC_BIN = target/release/$(NAME)
MINI_SRC_BIN = target/release/$(MINI_NAME)

ifneq (,$(wildcard config.mk))
    include config.mk
else
    $(error config.mk not found. Please run ./configure first.)
endif

ifeq ($(OS_NAME),windows)
    IS_WINDOWS := 1
    SRC_BIN = target/release/$(NAME).exe
    MINI_SRC_BIN = target/release/$(MINI_NAME).exe
endif

.PHONY: all build install uninstall clean setup help

all: build install setup
	@echo "Done"

build:
	@echo "Building Zoi targets: $(WITH_BIN) in release mode (commit: $(COMMIT_HASH))..."
ifeq ($(WITH_BIN),zoi)
	@ZOI_COMMIT_HASH=$(COMMIT_HASH) cargo build --bin zoi --release
else ifeq ($(WITH_BIN),zoi-mini)
	@ZOI_COMMIT_HASH=$(COMMIT_HASH) cargo build --bin zoi-mini --release
else
	@ZOI_COMMIT_HASH=$(COMMIT_HASH) cargo build --bin zoi --bin zoi-mini --release
endif
	@echo "Build complete for $(OS_NAME) ($(ARCH_NAME))."

install:
ifeq ($(IS_WINDOWS),1)
	@echo "Installing requested binaries to $(BINDIR)..."
	@if not exist "$(BINDIR)" mkdir "$(BINDIR)"
ifneq ($(WITH_BIN),zoi-mini)
	@copy /Y "$(SRC_BIN)" "$(BINDIR)\$(NAME).exe"
	@echo "Zoi installed successfully to $(BINDIR)\$(NAME).exe"
endif
ifneq ($(WITH_BIN),zoi)
	@copy /Y "$(MINI_SRC_BIN)" "$(BINDIR)\$(MINI_NAME).exe"
	@echo "Zoi Mini installed successfully to $(BINDIR)\$(MINI_NAME).exe"
endif
	@echo "Make sure '$(BINDIR)' is in your system's PATH."
else
	@echo "Installing requested binaries to $(BINDIR)..."
	@mkdir -p "$(BINDIR)"
ifneq ($(WITH_BIN),zoi-mini)
	@install -m 755 "$(SRC_BIN)" "$(BINDIR)/$(NAME)"
	@echo "Zoi installed successfully to $(BINDIR)/$(NAME)"
endif
ifneq ($(WITH_BIN),zoi)
	@install -m 755 "$(MINI_SRC_BIN)" "$(BINDIR)/$(MINI_NAME)"
	@echo "Zoi Mini installed successfully to $(BINDIR)/$(MINI_NAME)"
endif
	@echo "Make sure '$(BINDIR)' is in your shell's PATH."
endif

uninstall:
ifeq ($(IS_WINDOWS),1)
	@echo "Uninstalling binaries from $(BINDIR)..."
	@if exist "$(BINDIR)\$(NAME).exe" del /f "$(BINDIR)\$(NAME).exe"
	@if exist "$(BINDIR)\$(MINI_NAME).exe" del /f "$(BINDIR)\$(MINI_NAME).exe"
	@echo "Binaries uninstalled."
else
	@echo "Uninstalling binaries from $(BINDIR)..."
	@rm -f "$(BINDIR)/$(NAME)"
	@rm -f "$(BINDIR)/$(MINI_NAME)"
	@echo "Binaries uninstalled."
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
