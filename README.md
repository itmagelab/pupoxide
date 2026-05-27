# Pupoxide: Puppet for the Rust Era

Pupoxide is a high-performance, robust, and declarative configuration management tool inspired by Puppet, but reimagined for the modern Rust era.

> [!WARNING]
> **Experimental Project / Proof of Concept**
>
> This project is an architectural experiment in reimagining Puppet's ideas using Rust. It is **not ready for production use**, but is actively developing. We welcome [new contributors](doc/workflow.md)!

---

## 🚀 Why Pupoxide?

| Feature | Puppet | Pupoxide |
| :--- | :--- | :--- |
| **Language** | Ruby (slow, heavy) | **Rust** (maximum speed) |
| **Dependencies** | Requires Ruby runtime | **Zero dependencies** (single binary) |
| **Safety** | Dynamic typing | **Static-like safety** + Dependency Graph (DAG) |
| **DSL** | Custom, limited | **Rhai** (powerful, extensible scripting) |
| **Parallelism** | Limited | **Native parallelism** for independent resources |

> [!TIP]
> **Core Value**: Pupoxide automatically builds a dependency graph and executes unrelated tasks (e.g., installing htop and configuring nginx) simultaneously, providing a massive speed boost on large configurations.

---

## 📜 Manifest Examples (Rhai-based DSL)

Pupoxide uses the powerful and concise syntax of [Rhai](https://rhai.rs/).

```rust
// Install packages via the 'brew' module
import "brew" as b;
b::pkg(["htop", "wget"], #{ ensure: "present" });

// Create a directory with permissions
directory("/tmp/pupoxide/cache", #{
    ensure: "present",
    mode: "0755",
    owner: "admin"
});

// File with content and a dependency
file("/etc/motd", #{
    ensure: "present",
    content: "Welcome to Pupoxide Node!",
    require: "Directory[/tmp/pupoxide/cache]" // Executes AFTER directory creation
});

// Conditional logic based on system facts
if facts["os_family"] == "Darwin" {
    exec("mac-cleanup", #{
        command: "rm -rf ~/Library/Caches/*",
        only_if: "test -d ~/Library/Caches"
    });
}
```

---

## 💻 CLI Usage

### 1. Server-Less Mode (Local Application)

Applies configuration directly on the current machine. Ideal for deployment scripts or local setup.

```bash
# Apply a specific file
pupoxide run --file ./examples/environments/production/manifests/site.rhai

# Apply an entire environment (Puppet-like structure)
pupoxide --config ./examples/ apply --environment production

# Preview changes only (Dry-run)
pupoxide apply --environment production --dry-run
```

### 2. Agent-Server Mode (mTLS Security)

Secure architecture with automatic certificate generation and a three-phase bootstrap.

1. **Start the Master:**

    ```bash
    pupoxide master start --port 8080
    ```

2. **Registration Request (on Agent):**

    ```bash
    pupoxide agent --server http://master:8080 --node agent-01 --bootstrap
    ```

3. **Sign Certificate (on Master):**

    ```bash
    pupoxide master sign --node agent-01
    ```

4. **Regular Operation (mTLS):**

    ```bash
    pupoxide agent --server https://master:8080 --node agent-01
    ```

---

## 📂 Project Structure

Pupoxide encourages the **Roles and Profiles** pattern for code clarity.

```text
/etc/pupoxide/
├── environments/
│   └── production/
│       ├── manifests/
│       │   └── site.rhai      # Entry point (imports roles)
│       ├── role/              # Business logic (e.g., "web_server.rhai")
│       ├── profile/           # Technical stacks ("nginx_proxy.rhai")
│       ├── modules/           # Reusable components (services, packages)
│       └── data/              # Hierarchical data (YAML)
└── certs/                 # Store for mTLS certificates
```

---

## 🛠 Additional Tools

* **Graph Visualization**: `pupoxide graph --file site.rhai --style mermaid` — generates a dependency diagram.
*   **Serialization**: `mutex: "id"` — ensures resources in the same group run serially while maintaining global parallelism.
* **Rollback**: Every transaction is logged, allowing you to return the system to a previous state.

### 🔒 Mutex Groups (Serial Execution)

Some resources (like package managers) cannot run in parallel. Use the `mutex` attribute to serialize them:

```rust
// These will run one by one, even if both are ready
pkg("htop", #{ mutex: "brew" });
pkg("wget", #{ mutex: "brew" });

// This one remains independent and runs in parallel
file("/tmp/config", #{ ensure: "present" });
```

## 📖 Documentation

* [Project Vision](doc/vision.md)
* [Coding Conventions](doc/conventions.md)
* [Architecture Context](doc/architecture_context.md)

---
License: MIT
