# GitStatus

A fast, modern Rust tool that provides concise git repository status information. Perfect for shell prompts, scripts, and large repositories/monorepos.

## Features

- **Fast**: Pure Rust implementation using `gix` (Gitoxide)
- **Concise**: Shows branch, upstream, and change summary in minimal format
- **Modern**: Built with modern Rust patterns and best practices
- **Safe**: Proper error handling and memory safety
- **Configurable**: CLI options for different use cases

## Installation

### From Source

```bash
git clone https://github.com/stephenyu/gitstatus.git
cd gitstatus
cargo install --path .
```

### Using Cargo

If published to crates.io:

```bash
cargo install gitstatus
```

## Usage

### Basic Usage

```bash
# In any git repository
gitstatus
# Output: main origin/main ✓

# With changes
gitstatus
# Output: main origin/main +2~1-1
```

### Command Line Options

```bash
# Show help
gitstatus --help

# Specify a different repository path (also accepts a positional path)
gitstatus --path /path/to/repo
gitstatus /path/to/repo

# Show verbose error messages
gitstatus --verbose

# Show version
gitstatus --version

# Control untracked scanning (default: no)
gitstatus -u no|normal|all   # --all is equivalent to -u all

# Skip staged-change counting
gitstatus --no-staged        # or -S

# Read branch/upstream directly from .git (lower overhead, less robust)
gitstatus --direct-upstream  # or -U
```

## Output Format

The output consists of three parts separated by spaces:

1. **Current Branch**: Name of the current branch (or "HEAD" if detached)
2. **Upstream Branch**: Name of the upstream branch (if configured and different from current)
3. **Changes Summary**: Summary of repository changes

### Change Summary Symbols

- `✓` - Clean working directory
- `^N` - N staged changes (index differs from `HEAD^{tree}`)
- `~N` - N renames/copies between index and worktree
- `~N` - N modified/type-changed/conflict entries between index and worktree
- `-N` - N deleted files (index vs worktree)
- `+N` - N untracked files

Notes:
- The `~` symbol can appear twice: once for renames/copies and once for other modifications.
- Additions that are staged are counted in `^N`; before staging, they appear as untracked `+N`.

## Examples

```bash
# Clean repository on main branch tracking origin/main
$ gitstatus
main origin/main ✓

# Repository with staged, renamed, modified, deleted and untracked changes
$ gitstatus
main origin/main ^1~2~3-1+4

# Detached HEAD state
$ gitstatus
HEAD ✓

# Branch without upstream
$ gitstatus
feature-branch ✓
```

## What's New in v1.0.0

This version represents a complete modernization of the codebase:

### 🚀 **Modern Rust Patterns**
- **Better Error Handling**: Uses `anyhow` for context-rich error messages
- **Lightweight CLI**: Manual argument parsing for zero additional dependencies
- **Type Safety**: Structured data types instead of string manipulation
- **Memory Safety**: No more potential panics from string indexing

### 🏗️ **Improved Architecture**
- **Separation of Concerns**: Clear separation between data collection, processing, and output
- **Pure `gix`**: Eliminated external `git` command calls in favor of `gix` (Gitoxide)
- **Structured Status**: Uses `gix::status` platform and `tree_index_status()` for precise, fast diffs
- **Extensible Design**: Easy to add new features and status indicators

### ⚡ **Performance Improvements**
- **Fast default**: Avoid scanning untracked files by default (equivalent to `git -uno`)
- **Parallel checks**: Uses `gix` parallel feature to check tracked-file modifications efficiently
- **Minimal I/O**: No process spawning; tracked-only checks for prompt use are extremely fast
- **New flags**:
  - `--no-staged` (`-S`): skip staged-change counting when you want minimal status
  - `--direct-upstream` (`-U`): read branch/upstream directly from `.git` for lowest overhead (may miss config layering/worktrees)

### 🛡️ **Reliability**
- **Proper Error Propagation**: No more silent failures with `process::exit(1)`
- **Graceful Handling**: Better handling of edge cases (detached HEAD, no upstream, etc.)
- **Input Validation**: Validates repository paths and handles invalid UTF-8

### 🎯 **User Experience**
- **Clear Output**: Compact summary with consistent symbols (`^`, `~`, `-`, `+`, `✓`)
- **Verbose Mode**: Optional detailed error messages for debugging
- **Flexible Paths**: Can check status of any repository, not just current directory
- **Help/Version**: Built-in usage and `--version`

## How it’s fast (with gix)

By default, `gitstatus` is optimized for shell prompts and large repos:

- It compares `HEAD^{tree}` to the index for staged changes using `Repository::tree_index_status()`.
- It compares the index to the working tree using `Repository::status(...).into_index_worktree_iter(...)` with untracked disabled unless requested.
- Untracked mode mapping:
  - default / `-u no`: no untracked scan (no dirwalk)
  - `-u normal`: collapsed untracked
  - `-u all` or `--all`: full untracked listing

Additional flags affecting performance:
- `--no-staged` (`-S`): disables staged diff computation (`HEAD^{tree}` vs index)
- `--direct-upstream` (`-U`): avoids full config resolution by reading `.git/HEAD` and `.git/config` directly (less accurate in complex setups)
- Submodule checks and rename tracking are disabled by default for speed; they can be enabled later if needed.

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run with development features
cargo run -- --verbose

# Check formatting and lints
cargo fmt --check
cargo clippy
```

## Requirements

- Rust 1.70+ (specified in Cargo.toml)
- Git repository to analyze

## License

GPL-3.0-only

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. 