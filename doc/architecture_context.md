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
    *   **backup**: Boolean flag to enable/disable state snapshotting (default: `true`).
    *   **max_backup_size**: Optional limit in bytes for content snapshots.
*   **Ensure**: Enum for desired state (`Present`, `Absent`).
*   **ResourceProvider (Port)**: An `async_trait` that must be implemented by Infrastructure adapters.

### Application Layer (`src/application/`)
- `PupoxideEngine`: Interface to Rhai, manages resource collection and include-logic.
- `EnvironmentLoader`: Resolution of manifests and modules within an environment.
- `RollbackEngine`: Logic for generating inverse catalogs based on transaction history and backups.

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
    content: "Welcome!",
    backup: true,
    max_backup_size: 1024 * 1024 // 1MB limit
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
5. - **Application Layer**: orchestrates resource collection via Rhai, manages environments, and handles the Transaction lifecycle (Snapshot -> Apply -> Log).
- **Execution Flow**:
    1. Agent collects facts and sends them to Master.
    2. Master evaluates manifests and returns a compiled `Catalog`.
    3. Agent performs a **Snapshot** (collects current state and backups).
    4. Agent applies the catalog.
    5. Agent logs the **Transaction** for possible future rollback.

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

## 6. Agent-Server Authentication (mTLS with CSR Signing)

Pupoxide uses **Mutual TLS (mTLS) with dynamic certificate signing** for secure Agent-Master communication:

### Architecture

```
┌─────────────────────────────────────────────────┐
│ Phase 1: Bootstrap (No mTLS yet)                │
│                                                 │
│ Agent                          Master           │
│   │ POST /register              │               │
│   │ (CSR, node_id, token) ──────>               │
│   │                             │ Verify token  │
│   │                             │ Sign CSR      │
│   │ <────────────────────────────               │
│   │  Return signed cert.pem                     │
│   │ Save to /etc/pupoxide/agent.pem             │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│ Phase 2: Regular Operation (mTLS)               │
│                                                 │
│ Agent                          Master           │
│   │ TLS Handshake               │               │
│   │ (present signed cert) ──────>               │
│   │                             │ Verify cert   │
│   │ <──────────────────────────── CN = node_id  │
│   │ POST /catalog (encrypted)                   │
│   │ ──────────────────────────>                 │
│   │                             │ Generate cat. │
│   │ <────────────────────────────               │
│   │ Return catalog (encrypted)                  │
└─────────────────────────────────────────────────┘
```

### Bootstrap Flow (Phase 1)

**Agent generates CSR (Certificate Signing Request)**:
```rust
// src/interface/agent.rs
impl PupoxideAgent {
    pub async fn bootstrap(&self, bootstrap_token: String) -> Result<()> {
        // 1. Generate private key locally
        let private_key = rcgen::generate_simple_self_signed(vec![self.node_name.clone()])?;
        
        // 2. Create CSR
        let csr = rcgen::CertificateSigningRequest::new(
            rcgen::CertificateParams {
                common_name: self.node_name.clone(),
                ..Default::default()
            },
            &private_key,
        )?;
        
        let csr_pem = csr.serialize_pem()?;
        
        // 3. Send CSR to Master (UNENCRYPTED, but with bootstrap token)
        let client = reqwest::Client::new();
        let response: BootstrapResponse = client
            .post(format!("{}/bootstrap", self.server_url))
            .header("X-Bootstrap-Token", bootstrap_token)
            .json(&serde_json::json!({
                "node_id": self.node_name,
                "csr": csr_pem,
            }))
            .send()
            .await?
            .json()
            .await?;
        
        // 4. Save signed certificate and private key
        std::fs::write("/etc/pupoxide/agent.pem", response.certificate)?;
        std::fs::write("/etc/pupoxide/agent.key", private_key.serialize_pem())?;
        
        tracing::info!("Agent {} registered successfully", self.node_name);
        Ok(())
    }
}
```

**Master receives and signs CSR**:
```rust
// src/interface/server.rs
async fn bootstrap(
    State(state): State<Arc<MasterState>>,
    headers: HeaderMap,
    Json(payload): Json<BootstrapRequest>,
) -> Result<Json<BootstrapResponse>, ServerError> {
    // 1. Verify bootstrap token
    let token = headers
        .get("X-Bootstrap-Token")
        .ok_or(ServerError(StatusCode::UNAUTHORIZED, "Missing token".into()))?;
    
    if !state.verify_bootstrap_token(token.to_str()?) {
        return Err(ServerError(StatusCode::FORBIDDEN, "Invalid token".into()));
    }
    
    // 2. Parse CSR
    let csr = rcgen::CertificateSigningRequest::from_pem(&payload.csr)
        .map_err(|e| ServerError(StatusCode::BAD_REQUEST, e.to_string()))?;
    
    // 3. Sign with Master CA
    let signed_cert = state.ca_cert.sign_request(&csr)
        .context("Failed to sign certificate")?;
    
    // 4. Store node in database (mark as registered)
    state.db.register_agent(&payload.node_id, &signed_cert).await?;
    
    // 5. Return signed certificate
    Ok(Json(BootstrapResponse {
        certificate: signed_cert.serialize_pem()?,
        ca_certificate: state.ca_pem.clone(),
    }))
}
```

### Regular Operation (Phase 2)

Once agent has signed certificate, all subsequent connections use **mTLS**:

```rust
// src/interface/agent.rs - After bootstrap
impl PupoxideAgent {
    pub async fn fetch_catalog(&self) -> Result<Catalog> {
        let cert_pem = std::fs::read("/etc/pupoxide/agent.pem")?;
        let key_pem = std::fs::read("/etc/pupoxide/agent.key")?;
        
        let identity = reqwest::Identity::from_pem(&cert_pem, &key_pem)?;
        let client = reqwest::Client::builder()
            .identity(identity)
            .build()?;
        
        // All connections are now encrypted and authenticated
        client
            .post(format!("{}/catalog/{}/{}", 
                self.server_url, self.environment, self.node_name))
            .json(&facts)
            .send()
            .await?
            .json()
            .await
    }
}
```

**Master validates certificate during TLS handshake**:
```rust
// src/interface/server.rs - TLS middleware
async fn verify_client_cert(req: Request, next: Next) -> Result<Response, ServerError> {
    let cert_cn = req.extensions()
        .get::<ClientCertificate>()?
        .common_name();
    
    // Verify CN matches allowed nodes
    if !state.db.is_agent_registered(&cert_cn).await? {
        return Err(ServerError(StatusCode::UNAUTHORIZED, 
            format!("Agent {} not registered", cert_cn)));
    }
    
    Ok(next.run(req).await)
}
```

### Key Features

- ✅ **Two-Phase Security**: Bootstrap with token, then mTLS
- ✅ **Dynamic Certificate Generation**: Each agent gets unique signed cert
- ✅ **No Pre-shared Secrets**: Bootstrap token is single-use
- ✅ **Encryption in Transit**: All communication encrypted after bootstrap
- ✅ **Mutual Authentication**: Both Agent and Master verify each other
- ✅ **Revocation**: Admin can revoke agent by removing from DB
- ✅ **Self-contained**: Agent generates its own private key (never transmitted)

### Certificate Management

```
Master CA (long-lived):
  /etc/pupoxide/ca.pem        # CA public key
  /etc/pupoxide/ca.key        # CA private key (protected)

Agent Certificates (signed by Master CA):
  /etc/pupoxide/agent.pem     # Signed agent certificate
  /etc/pupoxide/agent.key     # Agent private key (local only)
```

### Technologies

| Component | Library | Reason |
|-----------|---------|--------|
| **CSR Generation** | `rcgen` | Pure Rust cert generation, no OpenSSL |
| **Server TLS** | `tokio-rustls` + `axum` | Native Rust, mTLS support |
| **Client TLS** | `reqwest` + `rustls` | Built-in certificate handling |
| **PEM Parsing** | `rustls-pemfile` | Read/write PEM files |

### Bootstrap Token Management

Bootstrap tokens should be:
- **Issued by Admin**: `pupoxide bootstrap --node-id agent-01`
- **Single-use**: Invalidated after first registration
- **Short-lived**: Expire after 1 hour (configurable)
- **Stored in DB**: Track which token was used by which agent

```rust
// src/infrastructure/bootstrap_token.rs
pub struct BootstrapToken {
    pub token: String,
    pub node_id: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub used_at: Option<i64>,
}
```

### Admin Commands

```bash
# Generate bootstrap token (one-time)
pupoxide bootstrap-token --node-id agent-node01 --ttl 3600
# Output: BOOTSTRAP_TOKEN_XXXXX

# On agent side:
pupoxide agent bootstrap --server master.example.com --node-id agent-node01 --token BOOTSTRAP_TOKEN_XXXXX

# On master side (view registered agents):
pupoxide agents list
# node_id          | cert_cn        | registered_at         | last_seen
# agent-node01     | agent-node01   | 2026-01-17 14:00:00  | 2026-01-17 14:05:00

# Revoke agent access:
pupoxide agents revoke --node-id agent-node01
```

### Future Extensions

- **Certificate Rotation**: Agents request new signed certs before expiry (e.g., every 30 days)
- **Revocation Lists (CRL)**: Master publishes list of revoked certs
- **OCSP**: Real-time certificate validity checking
- **Hardware Tokens**: Support for HSM-stored agent private keys


## 7. Development Conventions
*   **No Unwraps**: Use `Result` and `anyhow::Error`.
*   **Async**: Infrastructure layer is async (Tokio).
*   **Idempotency**: Always check existence/state before applying changes.
*   **Stable IDs**: Resource IDs must be deterministic (e.g., `Type[Path/Name]`).
