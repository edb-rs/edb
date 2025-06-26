# Development Document

This is a quick note for early-stage development.

## Before Pushing to GitHub

Please ensure the following commands pass if you have changed the code:

```rust
cargo check --all
cargo test --all --all-features
cargo +nightly fmt -- --check
cargo clippy --all --all-targets --all-features -- -D warnings
```

## Some Hints

### Git Commit Message

+ feat: A new feature for the user.
+ fix: A bug fix.
+ docs: Documentation only changes.
+ style: Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc).
+ refactor: A code change that neither fixes a bug nor adds a feature.
+ perf: A code change that improves performance.
+ test: Adding missing tests or correcting existing tests.
+ chore: Changes to the build process or auxiliary tools and libraries such as documentation generation.
+ ci: Changes to CI configuration files and scripts (e.g., GitHub Actions, CircleCI).
+ build: Changes that affect the build system or external dependencies (example scopes: gulp, broccoli, npm).
+ revert: Reverts a previous commit.
