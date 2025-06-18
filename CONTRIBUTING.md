# Contributing Guidelines

Thank you for considering contributing to the `rtp-midi` project!

## Development Workflow

1.  Fork the repository.
2.  Create a new branch for your feature or bug fix.
3.  Make your changes.
4.  Ensure your code is well-formatted and passes all checks (see Pre-commit Hooks).
5.  Submit a pull request to the `master` branch.

## Pre-commit Hooks

This project uses `pre-commit` to automatically format code and run checks before each commit.

### Setup

1.  Install `pre-commit`:
    ```bash
    pip install pre-commit
    ```
2.  Install the git hooks:
    ```bash
    pre-commit install
    ```

Now, `cargo fmt` and `cargo clippy` will run automatically on the files you've changed before you commit. If they fail, fix the reported issues and re-add the files to your commit. 