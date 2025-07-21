# Contributing

We'll be more than happy if you want to contribute to our project and make it better.

You can find the code on [GitLab](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi),
And if you don't have a GitLab account, you can easily sign up using GitHub.

## Building from Source

To develop and build Zoi, you'll need the following prerequisites:

*   **Rust**: Make sure you have a recent version of Rust and Cargo installed. You can find installation instructions at [rust-lang.org](https://www.rust-lang.org/tools/install).
*   **Make**: The `make` command is required for the Makefile-based build process.

### Using Make (Recommended)

1.  **Clean (Optional)**: If you have a previous build, clean the project directory:
    ```bash
    cargo clean
    ```
2.  **Configure**:
    ```bash
    ./configure
    ```
3.  **Build and Install**:
    ```bash
    make
    sudo make install
    ```
4.  **Install Shell Completions (Optional)**:
    ```bash
    make install-completions
    ```

### Using Build Scripts

If you don't have `make`, you can use the provided build scripts:

*   **For Linux/macOS**:
    ```bash
    ./build/build.sh
    ```
*   **For Windows (PowerShell)**:
    ```powershell
    ./build/build.ps1
    ```

After building, you can run the executable directly:

*   **Linux/macOS**: `./build/compiled/zoi`
*   **Windows**: `./build/compiled/zoi.exe`

### Zoi Commands

Once Zoi is installed, you can use the commands defined in `zoi.yaml` to manage the project. For example, to run the build command:

```bash
zoi run build
```

Or run `zoi run` to view the available commands.
