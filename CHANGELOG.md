## [0.2.0] - 2026-01-14

### 🚀 Features

- Initialize project structure with layered architecture, add core dependencies, tracing, and custom domain error handling.
- Implement declarative file resource management with `FsAdapter` and DSL macros.
- Implement modular DSL with Rhai, dependencies, and environment support
- Implement Puppet-like module system
- Implement client-server architecture (Master/Agent)
- Implement system facts collection and transmission (Iteration 8)
- Implement module dependencies with virtual boundaries and thread-safe engine refactor
- Improve run command with smart module resolution and update docs
- Add directory resource, implementation for fs_adapter, and auto-parent creation for files
- Implement selective rollback with CAS backup store and transaction orchestration
- Enable rollback for agent by sharing transaction logic
- Implement dry-run mode (--dry-run)
- Add support for owner, group and mode resource attributes
- Implement Hiera for hierarchical data lookup, integrating it into the Rhai engine with fact-based interpolation and a new error type.
- Add Exec resource for command execution
- Modify common rhai flow

### 🐛 Bug Fixes

- Resolve master panic
- *(dsl)* Prevent ID collision between Roles and Profiles

### 💼 Other

- Engine make more readable
- Native rhai imports and module dependency tracking

### 🚜 Refactor

- Enforce no-unwrap convention and fix engine panics
- Rename Hiera to Stash
- Cleanup examples directory and integrate into production environment
- Simplify PupoxideModuleResolver::resolve and add comments

### 📚 Documentation

- Add architecture_context.md for AI agent onboarding
- Add module dependency example
- Update architecture and README with rollback feature details
- Add backup and max_backup_size details to architecture context
- Actualize README.md with current DSL syntax and modules
- Add experimental project warning and contributor call
- Add dry-run mode instructions to README
- Update repo URL and add ownership example
- Update repo URL and add ownership example
- Remove comments from example rhai files
- Remove removed backup features from README
- Added cliff config

### 🧪 Testing

- Add manual verification manifest for permissions
- Add integration tests for Exec resource

### ⚙️ Miscellaneous Tasks

- Formatiing with cargo fmt command
- Aadded new rules for gitignore
- Update crates
- Replace by Default trait
- Remove backup/rollback features and bump version to v0.2.0
