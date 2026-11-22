# CodeDead IAM — Backend API (Rust / Actix Web / MongoDB)

REST API backend for **CodeDead IAM**, targeting **Rust Edition 2024 (`rustc 1.100.0`)**, **Actix Web 4**, and the official **MongoDB Rust Driver 3**.

## Module Architecture (Zero `mod.rs`)

- `src/web/controller/` — Thin HTTP controllers (`audit`, `auth`, `users`, `roles`, `permissions`, `organisations`, `profile` including self-service account deletion, `dashboard`, `health` returning `503` when DB readiness fails).
- `src/web/dto/` — Validated API request/response DTOs (`#[serde(deny_unknown_fields)]`, trimmed Unicode-length validation, bounded ID arrays).
- `src/services/` — Business rules, privilege-escalation protection (blocking non-SuperAdmins from managing accounts with permissions they do not hold), atomic `login`/`change_password` updates, multi-organisation membership management with optimistic concurrency (`version`), delegated admin `initial_owner_email` / `initial_member_ids` creation (requiring an active user account for the initial owner), idempotent delete auditing, strict tenant isolation & user-data minimization (active-account signup-email invitations capped at 100 pending per org, consent enforcement, non-active owner-promotion lock, and name redaction while `invited`), structured + persisted audit logging (including `organisation.invitation_declined`, `member_email`, and delegated-creation details), user-to-organisation membership lookup (`GET /api/v1/users/{id}/organisations`), permission-filtered dashboard analytics & MongoDB aggregations, cache invalidation, and session revocation / refresh-token cleanup (per-family pruning on rotation and immediate deletion on logout or account deletion).
- `src/repository/` — Domain-grouped repositories (`*_repository.rs`) and BSON DAOs (`dao/*.rs`) for `user`, `role`, `permission`, `organisation`, `auth`, and `audit` (with batched `$in` lookups and replica-set `WriteConflict` rotation handling).
- `src/middleware/` — JWT authentication, `require_permission` RBAC guards with bounded generation-tracked `PermissionCache`, double-submit CSRF validation, cross-worker shared bounded-memory rate limiting (both `/api/v1` and `/api/v1/auth`, with `CF-Connecting-IP` / `X-Forwarded-For` / `X-Real-IP` support when `TRUST_PROXY_HEADERS=true`), and request ID / security headers.
- `src/security/` — Argon2id password hashing (with dummy verification timing protection), EdDSA (Ed25519) JWT signing/verification, key loading with startup sign+verify self-check, and opaque token / SHA-256 helpers.
- `src/database/` — MongoDB connection, index initialization (case-insensitive collation on `organisations.name`, `refresh_tokens` `expires_at` / `revoked_at` TTL retention and `audit_logs` TTL retention with automatic drop when set to `0`), replica-set transaction detection, and idempotent multi-replica system RBAC seeding.
- `src/openapi.rs` & `src/routes.rs` — Route wiring and vendored OpenAPI 3.1 / Swagger UI documentation (disabled by default in production via `ENABLE_SWAGGER_UI`).

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
