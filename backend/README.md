# CodeDead IAM — Rust Backend API

Actix Web + MongoDB REST API built with **Rust Edition 2024** targeting **`rustc 1.100.0`**.

## Module Architecture (Zero `mod.rs` Files)

- `src/web/controller/` — Thin Actix Web HTTP handlers (`auth`, `users`, `roles`, `permissions`, `profile`, `dashboard`, `health`).
- `src/web/dto/` — Validated request/response DTOs (`#[serde(deny_unknown_fields)]`, `validator::Validate`, `utoipa::ToSchema`).
- `src/services/` — Business logic, defense-in-depth RBAC checks, privilege-escalation guards, session revocation, and cache invalidation.
- `src/repository/` — Domain-grouped repositories and BSON DAOs:
  - `repository/user/{user_repository.rs, dao/user.rs}`
  - `repository/role/{role_repository.rs, dao/role.rs}`
  - `repository/permission/{permission_repository.rs, dao/permission.rs}`
  - `repository/auth/{refresh_token_repository.rs, dao/refresh_token.rs}`
- `src/middleware/` — JWT authentication (`JwtAuthMiddleware`), reusable RBAC permission guards (`require_permission`, `PermissionCache`), double-submit CSRF validation, sliding-window rate limiting, and request ID / security headers.
- `src/security/` — Ed25519 key management, EdDSA JWT signing/verification, Argon2id password hashing, and constant-time crypto helpers.
- `src/database/` — MongoDB client initialization, index verification, replica-set transaction detection, and idempotent system RBAC seeding.
- `src/openapi.rs` & `src/routes.rs` — OpenAPI 3.1 spec generation (vendored Swagger UI at `/swagger-ui/`) and route registration under `/api/v1`.

## Local Commands

```bash
cp .env.example .env

# Run API locally
cargo run

# Format, lint & test (unit + Actix HTTP + RBAC integration tests run without MongoDB)
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked

# Run live MongoDB integration test
MONGODB_TEST_URI=mongodb://localhost:27017 cargo test --test mongodb_integration_tests -- --nocapture

# Build optimized production release binary (opt-level=3, fat LTO, single CGU, stripped, panic=abort)
cargo build --release --locked
```

See [migrations/README.md](migrations/README.md) for MongoDB collections, indexes, and replica set requirements.
