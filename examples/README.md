# Pupoxide Examples

This directory contains ready-to-use examples for testing Pupoxide functionality.

## Directory Structure

- `environments/` - Example configuration environments (production with modules, profiles, roles)
- `certs/` - Certificate management directory (created at runtime)
  - `bootstrap_requests/` - Pending bootstrap CSRs
  - `agents/` - Registered agent certificates
  - `ca.pem` / `ca.key` - Master CA certificates
- `test_bootstrap.sh` - Automated test script for the complete bootstrap workflow

## Quick Start: Bootstrap Workflow Test

The easiest way to test Pupoxide's three-phase bootstrap with certificate-based authorization:

```bash
cd /Users/wilful/Git/pupoxide
bash examples/test_bootstrap.sh
```

This script will:

1. **Start Master Server** on port 8080
2. **List pending requests** (empty initially)
3. **Agent submits bootstrap request** (CSR without token)
4. **List pending requests** again (shows agent-01)
5. **Admin approves** the request and signs certificate
6. **Agent checks status** and retrieves certificate
7. **Verify** certificate files were created
8. **Clean up** (stop Master server)

### Expected Output

All steps should complete successfully with green checkmarks (✓):

```
✓ Bootstrap request submitted!
✓ Node 'agent-01' has been approved and registered
✓ Bootstrap approved!
✓ Certificate found
✓ Private key found
```

## Manual Testing

If you prefer to manually test each step:

### 1. Start Master Server

```bash
cargo run -- --config ./examples master start --port 8080
```

### 2. Submit Bootstrap Request (from another terminal)

```bash
cargo run -- --config ./examples agent \
  --server http://localhost:8080 \
  --node agent-01 \
  --environment production \
  --cert-dir ./examples/certs/agents/agent-01 \
  --bootstrap
```

### 3. List Pending Requests

```bash
cargo run -- --config ./examples master list
```

### 4. Admin Approves Request

```bash
cargo run -- --config ./examples master sign --node agent-01
```

Or reject with:

```bash
cargo run -- --config ./examples master reject --node agent-01
```

### 5. Agent Checks Status and Gets Certificate

```bash
cargo run -- --config ./examples agent \
  --server http://localhost:8080 \
  --node agent-01 \
  --environment production \
  --cert-dir ./examples/certs/agents/agent-01 \
  --check --check-timeout 10
```

### 6. Verify Certificates

```bash
ls -la ./examples/certs/agents/agent-01/
```

You should see:
- `agent.pem` - Agent certificate (signed by CA)
- `agent.key` - Agent private key
- `ca.pem` - CA certificate for verification

## Configuration Storage

All bootstrap and agent data is stored in the `examples/certs/` directory:

- `certs/ca.pem` / `certs/ca.key` - Master's Certificate Authority
- `certs/bootstrap_requests/` - Pending CSR requests (JSON files)
- `certs/agents/` - Registered agent certificates and metadata

These are all local files and safe to delete to reset the test.

## Agent Lock Mechanism

Pupoxide agents use an exclusive lock file to prevent concurrent execution:

- **Lock File**: `certs/agents/{node_id}.lock`
- **Behavior**: When an agent starts, it creates a lock file. If another instance tries to start, it waits (polls every 100ms) until the lock is released
- **Timeout**: Default 300 seconds (5 minutes) - second instance fails if lock isn't released within timeout
- **Auto-cleanup**: Lock file is automatically removed when agent exits

### Testing Lock Mechanism

```bash
bash examples/test_agent_lock.sh
```

This test:
1. Starts the Master server
2. Bootstraps agent-01 and approves it
3. Starts first agent instance (acquires lock)
4. Starts second agent instance (waits for lock)
5. Verifies both complete without concurrent execution

Expected output:
```
✓ Lock file exists
✓ Both agents completed
✓ Lock mechanism ensured sequential execution
```

## Cleanup

To reset and run tests again:

```bash
rm -rf ./examples/certs
```


## Security Notes

- No bootstrap tokens required - CSR is proof of identity
- All communication after bootstrap is encrypted with mTLS
- Admin must explicitly approve each bootstrap request
- Certificates are stored locally with proper permissions (600 on keys)
