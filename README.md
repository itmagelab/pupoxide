# Pupoxide (Puppet for the Rust Era)

Pupoxide is a high-performance, memory-safe, and declarative configuration management tool inspired by Puppet, built with Rust and the Rhai scripting engine.

## Key Features
- **Declarative DSL**: Use [Rhai](https://rhai.rs/) scripts for clear, modular manifests.
- **Hexagonal Architecture**: Core logic isolated from system-specific implementation.
- **Environment & Module Support**: Organize configuration in environments like `production` or `staging`.
- **Idempotency**: Resources ensure the desired system state without redundant actions.

## Installation

```bash
git clone https://github.com/wilful/pupoxide.git
cd pupoxide
cargo build --release
```

## Quick Start

### 1. Run a single manifest
You can execute any `.rhai` script directly:

```bash
cargo run -- run --file ./examples/environments/production/manifests/site.rhai
```

### 2. Apply an environment
Apply all manifests from a specific environment using the Puppet-like directory structure:

```bash
cargo run -- apply --environment production --config ./examples
```
*Note: The `--config` flag points to the directory containing the `environments` folder. Defaults to `/etc/pupoxide`.*

## Example Manifest (`site.rhai`)

```rust
// Define a file resource
let config = file("/tmp/hello.txt", present(), "Hello from Pupoxide!");

// Return it (or an array of resources) to apply
[config]
```

## Directory Structure
Pupoxide follows a modular structure for easier management:

```text
/etc/pupoxide/
  environments/
    production/
      manifests/
        site.rhai      # Entry point
      modules/
        nginx/         # Future module support
```

## Documentation
- [Project Vision](doc/vision.md)
- [Coding Conventions](doc/conventions.md)
- [Development Workflow](doc/workflow.md)
- [Task List](doc/tasklist.md)
