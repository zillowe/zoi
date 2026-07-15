/// Contains auto-generated command templates for external package managers.
///
/// These templates are generated from `managers.json` during the build process
/// and are used by Zoi to interface with tools like `apt`, `brew`, and `cargo`.
include!(concat!(env!("OUT_DIR"), "/generated_managers.rs"));
