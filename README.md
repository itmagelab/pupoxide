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

Pupoxide can operate in a Master/Agent architecture with secure mutual TLS (mTLS) authentication.

#### Two-Phase Security Model

Pupoxide implements a secure two-phase bootstrap process:

**Phase 1: Bootstrap** (without mTLS)
- Agent generates a private key and Certificate Signing Request (CSR)
- Agent sends CSR to Master with a one-time bootstrap token
- Master verifies the token and signs the certificate
- Agent saves the signed certificate locally

**Phase 2: Regular Operation** (with mTLS)
- Agent uses the signed certificate for all communication
- Master verifies the certificate during TLS handshake
- All communication is encrypted and mutually authenticated

#### Usage

**Start the Master Server:**

```bash
cargo run -- --config ./examples master --port 8080
```

The Master will automatically:
- Generate a CA certificate at `/etc/pupoxide/ca.pem`
- Store the CA private key at `/etc/pupoxide/ca.key`
- Accept agent bootstrap requests with valid tokens

**Bootstrap an Agent** (Phase 1 - one time only):

```bash
# Generate a bootstrap token on the master (via admin command)
# This would be: pupoxide bootstrap-token --node-id agent-01 --ttl 3600
# For now, you need to generate it manually

BOOTSTRAP_TOKEN="your-generated-token-here"

cargo run -- --config ./examples agent \
  --server http://localhost:8080 \
  --node my-node \
  --environment production \
  --bootstrap \
  --token "$BOOTSTRAP_TOKEN"
```

This will:
1. Generate a private key locally (never sent to server)
2. Create a CSR and send it to the Master with the bootstrap token
3. Receive a signed certificate from the Master
4. Save certificate and key to `/etc/pupoxide/agents/my-node/`

**Run the Agent** (Phase 2 - regular operation):

Once bootstrap is complete, run the agent with the signed certificate:

```bash
cargo run -- --config ./examples agent \
  --server https://localhost:8080 \
  --node my-node \
  --environment production
```

The agent will:
1. Load the signed certificate and private key
2. Connect to Master via mTLS
3. Request the catalog for the node
4. Apply the configuration

#### Security Features

✅ **Mutual TLS (mTLS)**: Both agent and master verify each other  
✅ **One-time Bootstrap Token**: Single-use token prevents replay attacks  
✅ **Dynamic Certificates**: Each agent gets a unique signed certificate  
✅ **Private Key Protection**: Private keys never leave the agent (0600 permissions)  
✅ **Encrypted Communication**: All post-bootstrap communication is encrypted

#### Complete Workflow Example

```bash
# Terminal 1: Start the Master server
cargo run -- --config ./examples master --port 8080

# Terminal 2: Bootstrap the agent (one time, Phase 1)
# Generate a token (admin would do this)
BOOTSTRAP_TOKEN=$(uuidgen)  # or use: $(date +%s | sha256sum | head -c 32)

# Run bootstrap command
cargo run -- --config ./examples agent \
  --server http://localhost:8080 \
  --node agent-01 \
  --environment production \
  --bootstrap \
  --token "$BOOTSTRAP_TOKEN"

# Certificate saved to: /etc/pupoxide/agents/agent-01/

# Terminal 2: Run the agent normally (Phase 2, repeated)
# After bootstrap is complete, run the agent as usual
cargo run -- --config ./examples agent \
  --server https://localhost:8080 \
  --node agent-01 \
  --environment production

# The agent will now use mTLS for secure communication
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
