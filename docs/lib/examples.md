---
title: Library Examples
description: Practical examples of how to use the Zoi library in your Rust code.
---

This page provides practical, copy-paste-ready examples for using Zoi's library features.

## Basic Package Installation

This example demonstrates the most common use case: installing a package using the default settings. We will install the `hello` package, automatically confirming any prompts.

### Code

```rust
use zoi::{install, InstallMode, InstallReason};

fn main() {
    let package_source = "hello";

    println!("Attempting to install '{}'...", package_source);

    // Call the install function with default preferences.
    let result = zoi::install(
        package_source,
        InstallMode::PreferBinary, // Try to get a binary, fall back to source.
        false,                     // Don't force re-installation.
        InstallReason::Direct,     // This is a direct request from the user.
        true,                      // Automatically say "yes" to any prompts.
    );

    // Check the result and print a message.
    match result {
        Ok(()) => println!("'{}' was installed successfully!", package_source),
        Err(e) => eprintln!("Error installing '{}': {}", package_source, e),
    }
}
```

### Explanation

1.  **Import necessary items:** We bring `install`, `InstallMode`, and `InstallReason` into scope from the `zoi` crate.
2.  **Define parameters:** We set the `package_source` to `"hello"` and configure the installation mode. `InstallMode::PreferBinary` is a safe default. Setting `yes: true` ensures the function runs non-interactively.
3.  **Call `zoi::install`:** We pass our parameters to the function.
4.  **Handle the result:** The `install` function returns a `Result`. We use a `match` statement to check if the installation was successful or if an error occurred, and we print an appropriate message.

## Forcing an Install from Source

Sometimes you may want to ensure a package is built from its source code, for example, to apply a patch or enable a specific feature. You can do this by using `InstallMode::ForceSource`.

### Code

```rust
use zoi::{install, InstallMode, InstallReason};

fn main() {
    // 'git' is a good example of a package that can be built from source.
    let package_source = "git";

    println!("Attempting to build and install '{}' from source...", package_source);

    let result = zoi::install(
        package_source,
        InstallMode::ForceSource, // Force a source build.
        true,                     // Force re-installation even if it exists.
        InstallReason::Direct,
        true,
    );

    match result {
        Ok(()) => println!("'{}' was built and installed successfully!", package_source),
        Err(e) => eprintln!("Error building '{}': {}", package_source, e),
    }
}
```

This example will cause Zoi to find the source code for `git`, download it, and run the build commands specified in its `pkg.yaml` file, even if a pre-compiled binary is available.
