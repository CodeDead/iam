# CodeDead IAM — Secure User, Role & Permission Management System

A production-oriented, full-stack Role-Based Access Control (RBAC) platform built with a **Rust (Edition 2024 / `rustc 1.100.0`) + Actix Web 4 + MongoDB** REST API backend and a **React 19 (JavaScript ES6+) + React Router 7 + Mantine 9 + Vite 8** administration frontend.

---

## 1. Architecture Overview

The repository contains two independently structured projects communicating exclusively over a versioned JSON REST API (`/api/v1`) with short-lived **EdDSA (Ed25519) JWT access tokens** and **HttpOnly rotating refresh-token cookies** protected by **double-submit CSRF validation**.

```text
┌──────────────────────────────┐        HTTPS / JSON + Bearer JWT        ┌────────────────────────────────────┐
│  React 19 + Mantine 9 UI     │ ──────────────────────────────────────► │  Rust / Actix Web 4 REST API       │
│  - In-memory Access Token    │        + HttpOnly Refresh Cookie        │  - RequestId & Security Headers    │
│  - Axios Refresh Queue       │        + X-CSRF-Token Header            │  - Rate Limiting & Strict CORS     │
│  - Auth & Permission Context │ ◄────────────────────────────────────── │  - JwtAuthMiddleware               │
└──────────────────────────────┘                                         │  - RequirePermission Guards        │
                                                                         │  - Controllers → Services → Repos  │
                                                                         └──────────────────┬─────────────────┘
                                                                                            │ Official MongoDB Driver 3
                                                                                            ▼
                                                                         ┌────────────────────────────────────┐
                                                                         │  MongoDB (Replica Set rs0)         │
                                                                         │  - users, roles, permissions,      │
                                                                         │    refresh_tokens, audit_logs      │
                                                                         └────────────────────────────────────┘
```

### Layered Backend Separation

- **Controllers (`web/controller/*.rs`)**: Thin HTTP adapters; extract DTOs/params, attach cookies, call services, and format `ApiResponse`. Never access MongoDB or embed business logic.
- **DTOs (`web/dto/*.rs`)**: Strict separation between MongoDB BSON DAOs and API request/response contracts (`#[serde(deny_unknown_fields)]` blocks mass assignment; password hashes are never serialized).
- **Services (`services/*.rs`)**: Business rules, DTO validation, defense-in-depth permission & privilege-escalation checks, structured `tracing` + persistent MongoDB audit logging (`auth_backend::audit`), cache invalidation, and session revocation.
- **Repositories & DAOs (`repository/*/*_repository.rs`, `repository/*/dao/*.rs`)**: Trait-based MongoDB persistence with explicit indexes, transaction support, and BSON data access objects.
- **Module Convention**: Modern Rust 2024 directory modules — **zero `mod.rs` files** exist anywhere in the project.
- **Safety Lints & Release Optimization**: `unsafe_code = "forbid"`, `clippy::{unwrap_used, expect_used, panic} = "deny"`, and an optimized `[profile.release]` (`opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`, `panic = "abort"`).

### Frontend Highlights

- **Pure JavaScript (ES6+)** with React 19, React Router 7, Mantine UI 9, Vite 8, and Axios — no TypeScript and no TanStack packages.
- **Secure Token Lifecycle**: Access tokens live exclusively in JS memory (never `localStorage`), refresh tokens live in `HttpOnly` cookies, and concurrent `401` responses coalesce into a single `/api/v1/auth/refresh` request.
- **RBAC UX & Security Dashboard**: Navigation links, routes, Audit Logs (`/audit-logs`), profile Account Access Summary, and the interactive Dashboard (KPI cards, 7-day stacked SVG security/audit activity chart, user status donut chart, role & permission distribution bars, token-reuse alert banner, recent audit feed, and active session counts) are strictly guarded by user permissions both server-side and client-side; logging out clears route history state so a subsequent user never inherits a restricted redirect.
- **UX, Resilience & Accessibility**: Automatic OS color scheme detection (`defaultColorScheme="auto"`) with manual toggle, offline/timeout/gateway/non-JSON API error handling (`api.js` + `nginx.conf` `/api/` `503` JSON fallback), semantic `<main>` landmarks across all routes, meta description, and `public/robots.txt`.

---

## 2. Project Structure

```text
.
├── .github/
│   └── workflows/
│       ├── backend-ci.yml
│       ├── frontend-ci.yml
│       └── docker-publish.yml
├── deploy/
│   ├── README.md
│   ├── swarm-stack.yml
│   └── portainer.env.example
├── docker-compose.yml
├── mongo/
│   └── mongod.dev.conf
├── docs/
│   └── DEPLOYMENT.md
├── backend/
│   ├── README.md
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── Dockerfile
│   ├── .env.example
│   ├── config/default.toml
│   ├── migrations/README.md
│   ├── src/
│   │   ├── main.rs, lib.rs, app.rs, routes.rs, openapi.rs
│   │   ├── config.rs + config/settings.rs
│   │   ├── common.rs + common/{errors,responses,pagination,validation}.rs
│   │   ├── database.rs + database/{mongodb,seed}.rs
│   │   ├── security.rs + security/{keys,password,jwt,csrf}.rs
│   │   ├── middleware.rs + middleware/{authentication,authorization,csrf,rate_limit,request_id}.rs
│   │   ├── web.rs
│   │   ├── web/controller.rs + web/controller/{audit,auth,users,roles,permissions,profile,dashboard,health}.rs
│   │   ├── web/dto.rs + web/dto/{audit,auth,users,roles,permissions,profile,dashboard,health}.rs
│   │   ├── services.rs + services/{audit,auth,users,roles,permissions,profile,dashboard}.rs
│   │   ├── repository.rs
│   │   ├── repository/user.rs + repository/user/{user_repository.rs, dao.rs, dao/user.rs}
│   │   ├── repository/role.rs + repository/role/{role_repository.rs, dao.rs, dao/role.rs}
│   │   ├── repository/permission.rs + repository/permission/{permission_repository.rs, dao.rs, dao/permission.rs}
│   │   ├── repository/auth.rs + repository/auth/{refresh_token_repository.rs, dao.rs, dao/refresh_token.rs}
│   │   ├── repository/audit.rs + repository/audit/{audit_log_repository.rs, dao.rs, dao/audit_log.rs}
│   │   └── test_support.rs
│   └── tests/
│       ├── audit_log_tests.rs
│       ├── auth_tests.rs
│       ├── dashboard_tests.rs
│       ├── profile_tests.rs
│       ├── user_tests.rs
│       ├── role_tests.rs
│       ├── permission_tests.rs
│       └── mongodb_integration_tests.rs
└── frontend/
    ├── README.md
    ├── package.json
    ├── package-lock.json
    ├── Dockerfile
    ├── nginx.conf
    ├── security-headers.inc
    ├── vite.config.js
    ├── postcss.config.cjs
    ├── eslint.config.js
    ├── .prettierrc
    ├── .env.example
    ├── public/robots.txt
    └── src/
        ├── main.jsx, App.jsx, theme.js
        ├── config/api.js
        ├── contexts/{contextInstances,AuthContext,PermissionContext}.jsx
        ├── hooks/{useAuth,usePermissions}.js
        ├── services/{api,auditService,authService,userService,roleService,permissionService,profileService,dashboardService}.js
        ├── components/{common,navigation,forms}/
        ├── layouts/{AuthLayout,DashboardLayout}.jsx
        ├── routes/{AppRoutes,ProtectedRoute}.jsx
        ├── features/{audit,auth,dashboard,profile,users,roles,permissions}/
        ├── utils/{cookies,errors,formatters}.js
        └── test/
```

See [backend/README.md](backend/README.md) and [frontend/README.md](frontend/README.md) for subproject-specific guides.

---

## 3. MongoDB Schema & Relationships

| Collection | Key Fields | Indexes |
| --- | --- | --- |
| `users` | `_id`, `first_name`, `last_name`, `email`, `password_hash`, `status`, `email_verified`, `roles: [ObjectId]`, `token_version`, `profile`, `created_at`, `updated_at`, `last_login_at` | Unique `email` (`email_unique`), `status` (`status_idx`), `roles` (`roles_idx`), `created_at` (`created_at_desc`) |
| `roles` | `_id`, `name`, `description`, `permissions: [ObjectId]`, `is_system`, `created_at`, `updated_at` | Unique `name` (`role_name_unique`), `permissions` (`permissions_idx`), `is_system` (`is_system_idx`) |
| `permissions` | `_id`, `name` (`resource.action`), `resource`, `action`, `description`, `is_system`, `created_at`, `updated_at` | Unique `name` (`permission_name_unique`), compound unique `(resource, action)` (`resource_action_unique`), `is_system` (`perm_is_system_idx`) |
| `refresh_tokens` | `_id`, `user_id`, `token_hash` (SHA-256), `family_id`, `expires_at`, `revoked_at`, `replaced_by_hash`, `user_agent`, `ip_address`, `created_at` | Unique `token_hash` (`token_hash_unique`), `user_id` (`user_id_idx`), `family_id` (`family_id_idx`), `(family_id, revoked_at)` (`family_revoked_at_idx`), TTL on `expires_at` (`expires_at_ttl`), TTL on `revoked_at` (`revoked_at_ttl`) |
| `audit_logs` | `_id`, `action`, `resource_type`, `resource_id`, `resource_name`, `actor_id`, `actor_email`, `summary`, `details`, `created_at` | `created_at` (`audit_created_at_desc`), TTL on `created_at` (`audit_created_at_ttl`, configurable via `AUDIT_LOG_RETENTION_DAYS`), `(action, created_at)` (`audit_action_created_at`), `(resource_type, created_at)` (`audit_resource_type_created_at`), `(actor_id, created_at)` (`audit_actor_id_created_at`) |

- **Relationships**: Users reference multiple `roles` by `ObjectId`; roles reference multiple `permissions` by `ObjectId`. Effective permissions are computed dynamically as the union of permissions across all assigned roles.
- **Seeded System Roles**: `Super Administrator`, `Administrator`, `Moderator`, `User`.
- **Seeded System Permissions**: `users.{read,create,update,delete,assign_roles,manage_self_roles}`, `roles.{read,create,update,delete}`, `permissions.{read,create,update,delete}`, `audit_logs.read`, `profile.{read,update,delete}` (newly-introduced system permissions are automatically backfilled onto existing system roles on startup).

See [backend/migrations/README.md](backend/migrations/README.md) for indexing and replica-set transaction details.

---

## 4. Authentication, JWT Lifecycle & RBAC Security

1. **Password Security & Timing Protection**: Argon2id (`argon2 0.6`, `Algorithm::Argon2id`, `V0x13`, 19 MiB, 2 iterations) executed off the Actix event loop via `tokio::task::spawn_blocking`. Unknown-email logins execute a dummy Argon2id verification to prevent timing-based user enumeration, and all failed login attempts are recorded as `auth.login_failed` audit events.
2. **Access JWTs**: Asymmetric **EdDSA (Ed25519)** signed tokens (`jsonwebtoken 12`, `ed25519-dalek 3`, default 15m TTL) validating signature, `iss`, `aud`, `exp`, `nbf`, `token_type == "access"`, and `tv` (user `token_version`, incremented immediately upon role changes, account deactivation, or password changes). Permissions are **never** trusted blindly from JWT claims; they are resolved via a short-TTL `PermissionCache` explicitly invalidated immediately upon any user role, role permission, or permission mutation.
3. **Refresh Token Rotation, Reuse Detection & Cleanup**: Opaque 32-byte random tokens stored as SHA-256 hashes in MongoDB, transmitted via `HttpOnly; SameSite=Lax` cookies (`Secure` in production). Each refresh atomically rotates the token within a `family_id` (using MongoDB transactions when connected to a replica set) and prunes older revoked tokens in that family to at most **5** recent entries so repeated page reloads cannot grow the collection unboundedly. Presenting an already-rotated token triggers **reuse detection**, revoking the entire token family immediately. Revoked tokens are automatically purged by MongoDB's `revoked_at` TTL index (`REVOKED_TOKEN_RETENTION_SECONDS`, default `3600` seconds), expired tokens are purged via the `expires_at` TTL index, and logout or account/user deletion hard-deletes session tokens immediately.
4. **CSRF Protection**: Double-submit cookie pattern (`csrf_token` cookie + `X-CSRF-Token` header verified with constant-time comparison via `subtle`).
5. **Privilege Escalation Defenses** (enforced in both HTTP middleware and services):
   - Registration strictly assigns only the `User` role and rejects unexpected fields (`#[serde(deny_unknown_fields)]`).
   - Users cannot modify their own account status, delete themselves via administration endpoints, or modify their own roles unless explicitly holding `users.manage_self_roles`. Self-service account deletion (`DELETE /api/v1/profile`, requiring `profile.delete`) requires current password confirmation, revokes all sessions, and still protects the last active `Super Administrator`.
   - Non-Super-Administrators cannot assign `Super Administrator` or any role containing permissions they do not personally hold.
   - The last active `Super Administrator` cannot be disabled, locked, demoted, or deleted.
   - Protected system roles and system permissions cannot be deleted or renamed.

---

## 5. REST API Reference (`/api/v1`)

Interactive OpenAPI 3.1 Swagger UI (`utoipa 6` + vendored `utoipa-swagger-ui 10`) is enabled by default in development and disabled by default in production (`ENABLE_SWAGGER_UI`):
- `GET /swagger-ui/`
- `GET /api/v1/openapi.json`

### Health
- `GET /health` — Liveness probe
- `GET /health/ready` — Readiness probe (verifies MongoDB connectivity)

### Authentication
| Method | Endpoint | Auth / Permission | Description |
| --- | --- | --- | --- |
| `POST` | `/api/v1/auth/register` | Public | Register a standard `User` account |
| `POST` | `/api/v1/auth/login` | Public | Authenticate & issue access JWT + refresh cookie |
| `POST` | `/api/v1/auth/refresh` | Refresh cookie + CSRF | Rotate refresh token & issue new access JWT |
| `POST` | `/api/v1/auth/logout` | Refresh cookie + CSRF | Delete token family & clear cookies |
| `GET` | `/api/v1/auth/me` | Bearer JWT | Get authenticated user & effective permissions |

### Users, Roles, Permissions, Audit Logs, Profile & Dashboard
| Method | Endpoint | Required Permission |
| --- | --- | --- |
| `GET` | `/api/v1/dashboard/stats` | Authenticated (strictly filtered per permission: `users.read`, `roles.read`, `permissions.read`, `audit_logs.read`; always includes caller's own `my_active_sessions`) |
| `GET` / `PUT` / `DELETE` | `/api/v1/profile` | `profile.read` / `profile.update` / `profile.delete` |
| `PATCH` | `/api/v1/profile/password` | `profile.update` |
| `GET` / `POST` | `/api/v1/users` | `users.read` / `users.create` |
| `GET` / `PUT` / `DELETE` | `/api/v1/users/{id}` | `users.read` / `users.update` / `users.delete` |
| `PATCH` | `/api/v1/users/{id}/status` | `users.update` |
| `GET` / `PUT` | `/api/v1/users/{id}/roles` | `users.read` / `users.assign_roles` |
| `GET` | `/api/v1/users/{id}/permissions` | `users.read` |
| `GET` / `POST` | `/api/v1/roles` | `roles.read` / `roles.create` |
| `GET` / `PUT` / `DELETE` | `/api/v1/roles/{id}` | `roles.read` / `roles.update` / `roles.delete` |
| `GET` / `PUT` | `/api/v1/roles/{id}/permissions` | `roles.read` / `roles.update` |
| `GET` | `/api/v1/roles/{id}/users` | `roles.read` + `users.read` |
| `GET` / `POST` | `/api/v1/permissions` | `permissions.read` / `permissions.create` |
| `GET` / `PUT` / `DELETE` | `/api/v1/permissions/{id}` | `permissions.read` / `permissions.update` / `permissions.delete` |
| `GET` | `/api/v1/permissions/{id}/roles` | `permissions.read` + `roles.read` |
| `GET` | `/api/v1/audit-logs` | `audit_logs.read` |
| `GET` | `/api/v1/audit-logs/{id}` | `audit_logs.read` |

### Example Request & Response

```http
POST /api/v1/auth/login HTTP/1.1
Content-Type: application/json

{
  "email": "admin@example.com",
  "password": "ChangeMe123!"
}
```

```json
{
  "success": true,
  "data": {
    "access_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJFZERTQSJ9...",
    "token_type": "Bearer",
    "expires_in": 900,
    "expires_at": "2026-11-21T09:15:00Z",
    "csrf_token": "r4nd0m_csrf_value",
    "user": {
      "id": "67401a...",
      "first_name": "System",
      "last_name": "Administrator",
      "email": "admin@example.com",
      "status": "active",
      "roles": [{ "id": "67401a...", "name": "Super Administrator", "is_system": true }]
    },
    "permissions": ["permissions.create", "permissions.read", "roles.read", "users.read"]
  },
  "message": "Signed in successfully"
}
```

Error response example (`403 Forbidden`):

```json
{
  "success": false,
  "error": {
    "code": "INSUFFICIENT_PERMISSIONS",
    "message": "You do not have permission to perform this operation"
  }
}
```

---

## 6. Environment Configuration

Copy [backend/.env.example](backend/.env.example) to `backend/.env` and [frontend/.env.example](frontend/.env.example) to `frontend/.env`.

### Backend (`backend/.env`)
| Variable | Default (`config/default.toml`) | Description |
| --- | --- | --- |
| `APP_ENV` | `development` | `development` or `production` (production requires PEM keys & `COOKIE_SECURE=true`) |
| `SERVER_HOST` / `SERVER_PORT` | `127.0.0.1` / `8080` | Bind host and port |
| `MONGODB_URI` | `mongodb://localhost:27017` | MongoDB connection URI (standalone, replica set, or Atlas `mongodb+srv://`) |
| `MONGODB_DATABASE` | `user_management` | Configurable target MongoDB database name (override via `.env`, `docker-compose.yml`, or Portainer stack env) |
| `JWT_ISSUER` / `JWT_AUDIENCE` | `user-management-api` / `user-management-client` | JWT `iss` and `aud` validation claims |
| `JWT_ACCESS_TOKEN_TTL` / `JWT_REFRESH_TOKEN_TTL` | `900` / `604800` | Token lifetimes in seconds (15 min / 7 days) |
| `REVOKED_TOKEN_RETENTION_SECONDS` | `3600` | MongoDB TTL retention in seconds for revoked refresh tokens (1 hour; `0` disables `revoked_at` TTL); rotation also prunes to at most 5 revoked tokens per session family |
| `JWT_PRIVATE_KEY_PATH` / `JWT_PUBLIC_KEY_PATH` | *(empty in dev)* | Ed25519 PKCS#8/SPKI PEM key paths (mandatory in production) |
| `FRONTEND_URL` / `ALLOWED_ORIGINS` | `http://localhost:5173` | Explicit CORS origin allowlist (wildcards forbidden) |
| `COOKIE_SECURE` / `COOKIE_SAME_SITE` / `COOKIE_DOMAIN` | `false` / `Lax` / *(empty)* | Refresh & CSRF cookie attributes |
| `RATE_LIMIT_ENABLED` / `RATE_LIMIT_AUTH_PER_MINUTE` / `RATE_LIMIT_API_PER_MINUTE` | `true` / `30` / `300` | Per-IP rate limiting thresholds (bounded memory with stale bucket eviction; applies to both `/api/v1` and stricter `/api/v1/auth`) |
| `TRUST_PROXY_HEADERS` | `false` | When `true`, rate limiting and audit logs prefer Cloudflare's `CF-Connecting-IP` header, falling back to `X-Forwarded-For` / `X-Real-IP`; when `false`, uses only the direct socket peer IP |
| `ENABLE_SWAGGER_UI` | `true` (dev) / `false` (prod) | Controls whether `/swagger-ui/` and `/api/v1/openapi.json` are exposed (disabled by default in production) |
| `AUDIT_LOG_RETENTION_DAYS` | `90` | MongoDB TTL index retention period in days for `audit_logs` (`0` disables automatic expiration) |
| `PERMISSION_CACHE_TTL_SECONDS` | `30` | In-memory RBAC cache TTL in seconds (`0` disables caching for multi-replica deployments) |
| `REQUIRE_EMAIL_VERIFICATION` | `false` | When `true`, newly registered accounts start in `pending_verification` status |
| `BOOTSTRAP_SUPER_ADMIN_EMAIL` / `BOOTSTRAP_SUPER_ADMIN_PASSWORD` | `admin@example.com` / *(empty)* | Optional initial Super Administrator bootstrap |
| `RUST_LOG` | `info` | `tracing-subscriber` filter expression |

### Frontend (`frontend/.env`)
| Variable | Default | Description |
| --- | --- | --- |
| `VITE_API_BASE_URL` | `http://localhost:8080/api/v1` | Base URL for the Rust REST API |

---

## 7. Local Development & Docker Setup

### Option A: Docker Compose (Recommended)

```bash
docker compose up --build
```

Uses `mongo:latest` configured as a single-node replica set (`rs0`), the optimized Rust release container, and the Nginx frontend container:
- **Frontend**: `http://localhost:5173`
- **Backend API**: `http://localhost:8080/api/v1`
- **Swagger UI**: `http://localhost:8080/swagger-ui/`
- **Bootstrap Super Admin (dev compose defaults)**: `admin@example.com` / `ChangeMe123!`

### Option B: Manual Local Setup

1. Start MongoDB (`docker compose up -d mongodb`, or a local replica set `rs0` on `localhost:27017`).
2. Configure & run the backend:
   ```bash
   cd backend
   cp .env.example .env
   cargo run
   ```
3. Configure & run the frontend:
   ```bash
   cd frontend
   cp .env.example .env
   npm install
   npm run dev
   ```

### Option C: Production Docker Swarm / Portainer (MongoDB Atlas)

Deploy pre-built GHCR images with external Ed25519 Swarm secrets and MongoDB Atlas using [deploy/swarm-stack.yml](deploy/swarm-stack.yml) and [deploy/portainer.env.example](deploy/portainer.env.example). See [deploy/README.md](deploy/README.md) and [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for step-by-step instructions.

---

## 8. Testing, Linting & CI/CD

### Backend (`rustc 1.100.0`)

```bash
cd backend
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked

# Optional: run live MongoDB repository, index, and seeding integration test
MONGODB_TEST_URI=mongodb://localhost:27017 cargo test --locked --test mongodb_integration_tests -- --nocapture
```

### Frontend (Node.js 26 / Vite 8 / Vitest 5)

```bash
cd frontend
npm run format:check
npm run lint -- --max-warnings=0
npm test
npm run build
```

### GitHub Actions Workflows

Automated CI runs on pushes to `main` and `develop`, and on pull requests (`opened`, `synchronize`, `reopened`) targeting `main` or `develop`:
- **[.github/workflows/backend-ci.yml](.github/workflows/backend-ci.yml)**: Verifies zero `mod.rs` files, runs `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and executes the full unit, API/RBAC, and live `mongo:latest` integration test suite.
- **[.github/workflows/frontend-ci.yml](.github/workflows/frontend-ci.yml)**: Verifies JavaScript-only source and zero TanStack dependencies, runs Prettier, ESLint (`--max-warnings=0`), Vitest, and the Vite production build.
- **[.github/workflows/docker-publish.yml](.github/workflows/docker-publish.yml)**: Builds and publishes backend and frontend container images to `ghcr.io` on pushes to `main`, `v*.*.*` tags, or manual dispatch (ready for [deploy/swarm-stack.yml](deploy/swarm-stack.yml) in Portainer).

See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for production hardening instructions.
