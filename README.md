# Pupoxide (Puppet for the Rust Era)

Pupoxide is a high-performance, memory-safe, and declarative configuration management tool inspired by Puppet, built with Rust and the Rhai scripting engine.

> [!WARNING]
> **Experimental Project / Proof of Concept**
>
> This project is an attempt to reimplement the core ideas of Puppet using Rust. It is **not ready for production use** and serves primarily as an architectural experiment and a playground for ideas.
>
> We are actively looking for **contributors**! If you are interested in Rust, configuration management, or language design, please feel free to open issues or submit PRs.

## Key Features

- **Declarative DSL**: Use [Rhai](https://rhai.rs/) scripts for clear, modular manifests.
- **Hexagonal Architecture**: Core logic isolated from system-specific implementation.
- **Environment & Module Support**: Organize configuration in environments like `production` or `staging`.
- **Idempotency**: Resources ensure the desired system state without redundant actions.

## Installation

```bash
git clone https://gitverse.ru/itmagelab/pupoxide
cd pupoxide
cargo build --release
```

## Quick Start

### Dry-Run Mode (Preview)

You can preview changes without applying them by using the `--dry-run` flag with `run`, `apply`, or `agent` commands:

```bash
cargo run -- run --dry-run --file ./examples/environments/production/manifests/site.rhai
```

This will log actions as "Would ensure resource" instead of executing them.

### 1. Run a single manifest

You can execute any `.rhai` script directly:

```bash
cargo run -- run --file ./examples/environments/production/manifests/site.rhai
```


### 2. Apply an environment

Apply all manifests from a specific environment using the Puppet-like directory structure:

```bash
# Default config path is /etc/pupoxide
cargo run -- --config ./examples apply --environment production
```

### 3. Client-Server Mode

Pupoxide can operate in a Master/Agent architecture.

**Start the Master Server:**

```bash
cargo run -- --config ./examples master --port 8080
```

**Run the Agent:**

```bash
cargo run -- --config ./examples agent --server http://localhost:8080 --node my-node --environment production
```

## Example Manifest (`site.rhai`)

Pupoxide uses Rhai with a custom DSL. Resources are defined using object maps, and dependencies can be expressed using the `require` attribute or the arrow operator `->`.

```rust
// Load a module
// examples/environments/production/manifests/site.rhai

// Assign role to the current node
"demo".role;
```

## Directory Structure

Pupoxide follows a modular structure for easier management:

```text
[config_dir]/
  environments/
    production/
      manifests/
        site.rhai      # Entry point for the environment (imports roles)
      modules/
        systemd/       # Systemd module (manages units)
        brew/          # Homebrew module (manages packages)
        common/        # Common settings
        demo/          # Demo module
      role/            # Roles: Business logic abstraction
        demo.rhai      # Example Role
      profile/         # Profiles: Technology stack wrapper
        demo.rhai      # Example Profile

## Roles and Profiles Pattern
Pupoxide encourages the standard "Roles and Profiles" pattern to organize your code logic:

- **Roles**: High-level business abstractions (e.g., "Webserver", "Database Node").
  - **Constraints**: Roles can ONLY include Profiles. They cannot contain resources (`file`, `exec`) or include modules directly.
- **Profiles**: Technical stacks that wrap modules (e.g., "Nginx with PHP", "Postgres Hardened").
```

```rust
// role/demo.rhai
"demo".profile;
```

```rust
// profile/demo.rhai
"common".include;
"demo".include;
```

```rust
// modules/common/manifests/init.rhai
import "brew" as b;

// Install packages using the 'brew' module
b::pkg(["htop", "wget"], #{ ensure: "present" });

// Define a file
file("/tmp/.cacherc", #{
    ensure: "present",
    content: "Global settings"
});

// Conditional logic based on facts
if facts["os_family"] == "Darwin" {
    file("/tmp/pupoxide/mac_only_config", #{
        ensure: "present",
        content: "This is macOS"
    });
}
```

```

## Documentation
- [Project Vision](doc/vision.md)
- [Coding Conventions](doc/conventions.md)
- [Development Workflow](doc/workflow.md)
- [Task List](doc/tasklist.md)
