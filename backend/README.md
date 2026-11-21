# CodeDead IAM — Backend API (Rust / Actix Web / MongoDB)

REST API backend for **CodeDead IAM**, targeting **Rust Edition 2024 (`rustc 1.100.0`)**, **Actix Web 4**, and the official **MongoDB Rust Driver 3**.

## Module Architecture (Zero `mod.rs`)

- `src/web/controller/` — Thin HTTP controllers (`auth`, `users`, `roles`, `permissions`, `profile`, `dashboard`, `health`).
- `src/web/dto/` — Validated API request/response DTOs (`#[serde(deny_unknown_fields)]`).
- `src/services/` — Business rules, privilege-escalation protection, cache invalidation, and session revocation.
- `src/repository/` — Domain-grouped repositories (`*_repository.rs`) and BSON DAOs (`dao/*.rs`) for `user`, `role`, `permission`, and `auth`.
- `src/middleware/` — JWT authentication, `require_permission` RBAC guards, double-submit CSRF validation, rate limiting, and request ID / security headers.
- `src/security/` — Argon2id password hashing, EdDSA (Ed25519) JWT signing/verification, key loading, and opaque token / SHA-256 helpers.
- `src/database/` — MongoDB connection, index initialization, replica-set transaction detection, and system RBAC seeding.
- `src/openapi.rs` & `src/routes.rs` — Route wiring and vendored OpenAPI 3.1 / Swagger UI documentation.

## Quick Start

```bash
cp .env.example .env
# Optionally customize MONGODB_URI and MONGODB_DATABASE (defaults to user_management) in .env
cargo run
```

- API base URL: `http://127.0.0.1:8080/api/v1`
- Swagger UI: `http://127.0.0.1:8080/swagger-ui/`
- Health probes: `GET /health`, `GET /health/ready`

## Commands

```bash
# Formatting & strict Clippy (unsafe_code forbid, unwrap/expect/panic denied)
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings

# Run unit & Actix integration tests (no MongoDB required)
cargo test --locked

# Run live MongoDB repository, index, and seeding test
MONGODB_TEST_URI=mongodb://localhost:27017 cargo test --locked --test mongodb_integration_tests -- --nocapture

# Build optimized release binary (fat LTO, codegen-units=1, stripped, panic=abort)
cargo build --release --locked
```

See [migrations/README.md](migrations/README.md) for MongoDB collections, indexes, and replica-set transaction details, [../docs/DEPLOYMENT.md](../docs/DEPLOYMENT.md) for production Ed25519 key/Swarm secret setup, and the root [README.md](../README.md) for the full API reference.
