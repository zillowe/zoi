# Release Guidelines

This document outlines the release process for Zoi, which follows the Zillowe Foundation Versioning Method (ZFVM).

## Versioning

Zoi uses ZFVM for versioning. For detailed information on ZFVM, please refer to the [ZFVM documentation](https://zillowe.qzz.io/docs/methods/zfvm).

The version format is: `[Branch] [Status] [X.Y.Z] [Build (optional)]`

## Release Cycle and Branches

We operate on a cyclical release model with four main release branches, each serving a distinct purpose in our development and release pipeline.

### Branches

- **`Production`**: This release branch contains the official, stable release code that is deployed to end-users. Code on this branch is considered production-ready and has undergone thorough testing.

- **`Development`**: This is the main release branch for active development. All new features, bug fixes, and other changes are integrated into this branch.

- **`Public`**: This release branch holds pre-release versions of the code that are made available to users for testing before they are moved to `Production`. This allows for public feedback and validation.

- **`Special`**: This release branch is used for deploying builds with specific features for targeted testing or special purposes. These are not general public releases and are used to validate one or more specific changes in a controlled manner.

### Release Flow

The promotion of code from one release branch to another follows this sequence:

1.  **Development**: All new code is initially considered part of the `Development` release branch.
2.  **Public Testing**: Periodically, a version from `Development` is promoted to the `Public` release branch to create a pre-release for wider testing.
3.  **Production Release**: After a version has been stabilized in the `Public` branch, it is promoted to the `Production` branch for the official release. In some cases, a `Development` version can be promoted directly to `Production` if a public testing phase is not required.
4.  **Special Builds**: When needed, specific features can be released to the `Special` branch for isolated testing.
