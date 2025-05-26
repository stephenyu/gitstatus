# GitStatus

A fast, modern Rust tool that provides concise git repository status information. Perfect for shell prompts, scripts, and large repositories/monorepos.

## Features

- **Fast**: Pure Rust implementation using libgit2
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

# Specify a different repository path
gitstatus --path /path/to/repo

# Show verbose error messages
gitstatus --verbose
```

## Output Format

The output consists of three parts separated by spaces:

1. **Current Branch**: Name of the current branch (or "HEAD" if detached)
2. **Upstream Branch**: Name of the upstream branch (if configured and different from current)
3. **Changes Summary**: Summary of repository changes

### Change Summary Symbols

- `✓` - Clean working directory
- `+N` - N added files
- `~N` - N modified files  
- `-N` - N deleted files
- `rN` - N renamed files
- `tN` - N files with type changes

## Examples

```bash
# Clean repository on main branch tracking origin/main
$ gitstatus
main origin/main ✓

# Repository with changes
$ gitstatus
main origin/main +2~3-1

# Detached HEAD state
$ gitstatus
HEAD ✓

# Branch without upstream
$ gitstatus
feature-branch +1
```

## What's New in v0.3.0

This version represents a complete modernization of the codebase:

### 🚀 **Modern Rust Patterns**
- **Better Error Handling**: Uses `anyhow` for context-rich error messages
- **CLI Arguments**: Proper CLI parsing with `clap` derive macros
- **Type Safety**: Structured data types instead of string manipulation
- **Memory Safety**: No more potential panics from string indexing

### 🏗️ **Improved Architecture**
- **Separation of Concerns**: Clear separation between data collection, processing, and output
- **Pure libgit2**: Eliminated external `git` command calls for better performance
- **Structured Status**: Uses git2's native status API instead of parsing porcelain output
- **Extensible Design**: Easy to add new features and status indicators

### ⚡ **Performance Improvements**
- **Native Git Access**: Direct libgit2 usage is faster than spawning processes
- **Efficient Status Checking**: Only checks tracked files by default
- **Minimal Allocations**: Reduced string allocations and copying

### 🛡️ **Reliability**
- **Proper Error Propagation**: No more silent failures with `process::exit(1)`
- **Graceful Handling**: Better handling of edge cases (detached HEAD, no upstream, etc.)
- **Input Validation**: Validates repository paths and handles invalid UTF-8

### 🎯 **User Experience**
- **Better Symbols**: More intuitive change indicators (`~` for modified vs `+` for added)
- **Verbose Mode**: Optional detailed error messages for debugging
- **Flexible Paths**: Can check status of any repository, not just current directory
- **Help System**: Proper help and version information

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