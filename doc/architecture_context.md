# Pupoxide: Technical Context for AI Agents

This document provides a comprehensive technical overview of Pupoxide's internal workings, architecture, and DSL syntax. Use this as a reference when implementing new features or refactoring existing code.

## 1. Core Philosophy
Pupoxide is a Rust-native alternative to Puppet.
*   **Declarative**: Describe the *state*, not the *steps*.
*   **Idempotent**: Applying the same manifest twice has no side effects.
*   **Safe**: Uses Rust's memory safety and a Directed Acyclic Graph (DAG) for execution order.

## 2. Architecture (Hexagonal / Modular Monolith)
The project is divided into three main layers to separate business logic from implementation details.

### Domain Layer (`src/domain/`)
The "Source of Truth". Pure logic, no side effects.
*   **Resource**: An enum representing a system component (e.g., `File`).
    *   Each resource has a unique `id` (e.g., `File[/etc/motd]`) and a list of `dependencies`.
*   **Ensure**: Enum for desired state (`Present`, `Absent`).
*   **ResourceProvider (Port)**: An `async_trait` that must be implemented by Infrastructure adapters.

### Application Layer (`src/application/`)
Orchestrates the flow.
*   **PupoxideEngine**: Integrates the Rhai scripting engine.
    *   **ExecutionContext**: A thread-local structure containing resources, inclusion set, and module stack for safe concurrent execution.
    *   **Topological Sort**: Uses a DAG to sort resources based on dependencies (the `require` attribute or `->` operator).
    *   **include()**: Returns a `ModuleHandle` and provides automatic module-level boundary synchronization.
*   **EnvironmentLoader**: Resolves file paths for environments and modules.

### Infrastructure Layer (`src/infrastructure/`)
External dependencies and system interactions.
*   **FsAdapter**: Handles file system operations (create, delete, permissions). It is responsible for making the resource state real.

---

## 3. DSL Syntax (Rhai-based)

### Object Maps
Resources are defined using Rhai Object Maps (`#{...}`) for readability.
```rust
file("/etc/motd", #{
    ensure: "present",
    content: "Welcome!"
});
```

### Dependencies
Dependencies can be defined in two ways:
1.  **Attribute**: `require: resource_object` or `require: "File[/path]"`
2.  **Arrow Operator**: `resource1 -> resource2` (ensures 1 is applied before 2).

### Modules and Environments
*   **include("mod_name")**: Loads `modules/mod_name/manifests/init.rhai`.
*   **Global Collector**: All resources from the main manifest and all included modules are collected into a single pool and sorted together.

---

## 4. Execution Flow
1.  **CLI** receives `--environment` and `--config`.
2.  **EnvironmentLoader** finds the site manifest (`site.rhai`).
3.  **PupoxideEngine** executes the Rhai scripts.
    - Functions like `file()` and `include()` are called.
    - Resources populate the **Shared Collector**.
4.  **Engine** performs a **Topological Sort** on the collected resources.
5.  **Application Layer** iterates through sorted resources and calls the appropriate **Infrastructure Adapter**.
6.  **Adapter** ensures the system state matches the resource definition.

## 5. Directory Structure
```text
.
├── environments/
│   └── production/
│       ├── manifests/
│       │   └── site.rhai      # Entry point
│       └── modules/
│           └── nginx/
│               └── manifests/
│                   └── init.rhai
├── src/
│   ├── domain/               # Logic & Models
│   ├── application/          # Engine & Orchestration
│   ├── infrastructure/       # System Adapters
│   └── interface/            # CLI
└── tests/                    # Integration Tests
```

## 6. Development Conventions
*   **No Unwraps**: Use `Result` and `DomainError`.
*   **Async**: Infrastructure layer is async (Tokio).
*   **Idempotency**: Always check existence/state before applying changes.
*   **Stable IDs**: Resource IDs must be deterministic (e.g., `Type[Path/Name]`).
