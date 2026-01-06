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

### Rollback (Undo)
Pupoxide can undo changes made to the system using the `rollback` command. It uses a selective backup system to restore original file contents.

```bash
# Rollback the last transaction
cargo run -- rollback

# Rollback a specific transaction
cargo run -- rollback --transaction-id tx_123456789
```

To enable rollback for a resource, use the `backup: true` parameter (enabled by default):
```rhai
file("/etc/motd", #{
    content: "Welcome to the server!",
    backup: true,
    max_backup_size: 1024 * 1024 // 1MB limit
});
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
include("common");

// Define a directory
directory("/var/www/html", #{ ensure: "present" });

// Define a file that depends on the directory and a module
file("/var/www/html/index.html", #{
    ensure: "present",
    content: "<h1>Hello from Pupoxide!</h1>",
    require: [directory("/var/www/html"), include("common")]
});

// Or use the arrow operator for clean dependency chains
include("nginx") -> file("/etc/nginx/sites-enabled/default", #{
    ensure: "present",
    content: "server { listen 80; }"
});
```

## Directory Structure
Pupoxide follows a modular structure for easier management:

```text
[config_dir]/
  environments/
    production/
      manifests/
        site.rhai      # Entry point for the environment
      modules/
        nginx/         # Module 'nginx'
          manifests/
            init.rhai  # Entry point for the module
```

## Documentation
- [Project Vision](doc/vision.md)
- [Coding Conventions](doc/conventions.md)
- [Development Workflow](doc/workflow.md)
- [Task List](doc/tasklist.md)
