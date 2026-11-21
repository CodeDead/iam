# CodeDead IAM — Secure User, Role & Permission Management System

A production-oriented, full-stack Role-Based Access Control (RBAC) platform built with a **Rust (Edition 2024 / rustc 1.100.0) + Actix Web + MongoDB** REST API backend and a **React 19 (JavaScript ES6+) + React Router 7 + Mantine 9 + Vite 8** administration frontend.

---

## 1. Architecture Overview

The repository contains two independently structured projects communicating exclusively over a versioned JSON REST API (`/api/v1`) with short-lived **EdDSA (Ed25519) JWT access tokens** and **HttpOnly rotating refresh-token cookies** protected by **double-submit CSRF validation**.

```text
┌──────────────────────────────┐        HTTPS / JSON + Bearer JWT        ┌────────────────────────────────────┐
│  React + Mantine Frontend    │ ──────────────────────────────────────► │  Rust / Actix Web REST API         │
│  - In-memory Access Token    │        + HttpOnly Refresh Cookie        │  - RequestId & Security Headers    │
│  - Axios Refresh Queue       │        + X-CSRF-Token Header            │  - Rate Limiting & Strict CORS     │
│  - Auth & Permission Context │ ◄────────────────────────────────────── │  - JwtAuthMiddleware               │
└──────────────────────────────┘                                         │  - RequirePermission Guards        │
                                                                         │  - Controllers → Services → Repos  │
                                                                         └──────────────────┬─────────────────┘
                                                                                            │ Official MongoDB Driver
                                                                                            ▼
                                                                         ┌────────────────────────────────────┐
                                                                         │  MongoDB (Replica Set rs0)         │
                                                                         │  - users, roles, permissions,      │
                                                                         │    refresh_tokens (TTL indexed)    │
                                                                         └────────────────────────────────────┘
```

### Layered Backend Separation

- **Controllers (`web/controller/*.rs`)**: Thin HTTP adapters; extract DTOs/params, attach cookies, call services, format `ApiResponse`. Never access MongoDB or embed business logic.
- **DTOs (`web/dto/*.rs`)**: Strict separation between MongoDB BSON DAOs and API request/response contracts (`#[serde(deny_unknown_fields)]` blocks mass assignment; password hashes are never serialized).
- **Services (`services/*.rs`)**: Business rules, DTO validation, defense-in-depth permission & privilege-escalation checks, cache invalidation, and session revocation.
- **Repositories & DAOs (`repository/*/*_repository.rs`, `repository/*/dao/*.rs`)**: Trait-based MongoDB persistence with explicit indexes, transaction support, and BSON data access objects.
- **Module Convention**: Modern Rust 2024 directory modules — **zero `mod.rs` files** exist anywhere in the project.

---

## 2. Project Structure

```text
.
├── .github/
│   └── workflows/
│       ├── backend-ci.yml
│       └── frontend-ci.yml
├── docker-compose.yml
├── mongo/
│   └── mongod.dev.conf
├── docs/
│   └── DEPLOYMENT.md
├── backend/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── Dockerfile
│   ├── README.md
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
│   │   ├── web/controller.rs + web/controller/{auth,users,roles,permissions,profile,dashboard,health}.rs
│   │   ├── web/dto.rs + web/dto/{auth,users,roles,permissions,profile,dashboard,health}.rs
│   │   ├── services.rs + services/{auth,users,roles,permissions,profile,dashboard}.rs
│   │   ├── repository.rs
│   │   ├── repository/user.rs + repository/user/{user_repository.rs, dao.rs, dao/user.rs}
│   │   ├── repository/role.rs + repository/role/{role_repository.rs, dao.rs, dao/role.rs}
│   │   ├── repository/permission.rs + repository/permission/{permission_repository.rs, dao.rs, dao/permission.rs}
│   │   ├── repository/auth.rs + repository/auth/{refresh_token_repository.rs, dao.rs, dao/refresh_token.rs}
│   │   └── test_support.rs
│   └── tests/
│       ├── auth_tests.rs
│       ├── user_tests.rs
│       ├── role_tests.rs
│       ├── permission_tests.rs
│       └── mongodb_integration_tests.rs
└── frontend/
    ├── package.json
    ├── package-lock.json
    ├── Dockerfile
    ├── nginx.conf
    ├── README.md
    ├── vite.config.js
    ├── postcss.config.cjs
    ├── eslint.config.js
    ├── .prettierrc
    ├── .env.example
    └── src/
        ├── main.jsx, App.jsx, theme.js
        ├── config/api.js
        ├── contexts/{contextInstances.js, AuthContext.jsx, PermissionContext.jsx}
        ├── hooks/{useAuth,usePermissions}.js
        ├── services/{api,authService,userService,roleService,permissionService,profileService,dashboardService}.js
        ├── components/{common,navigation,forms}/
        ├── layouts/{AuthLayout,DashboardLayout}.jsx
        ├── routes/{AppRoutes,ProtectedRoute}.jsx
        ├── features/{auth,dashboard,profile,users,roles,permissions}/
        ├── utils/{cookies,errors,formatters}.js
        └── test/
```

---

## 3. MongoDB Schema & Relationships

| Collection | Key Fields | Indexes |
| --- | --- | --- |
| `users` | `_id`, `first_name`, `last_name`, `email`, `password_hash`, `status`, `email_verified`, `roles: [ObjectId]`, `token_version`, `profile`, `created_at`, `updated_at`, `last_login_at` | Unique `email`, `status`, `roles`, `created_at` |
| `roles` | `_id`, `name`, `description`, `permissions: [ObjectId]`, `is_system`, `created_at`, `updated_at` | Unique `name`, `permissions`, `is_system` |
| `permissions` | `_id`, `name` (`resource.action`), `resource`, `action`, `description`, `is_system`, `created_at`, `updated_at` | Unique `name`, compound unique `(resource, action)`, `is_system` |
| `refresh_tokens` | `_id`, `user_id`, `token_hash` (SHA-256), `family_id`, `expires_at`, `revoked_at`, `replaced_by_hash`, `user_agent`, `ip_address`, `created_at` | Unique `token_hash`, `user_id`, `family_id`, TTL on `expires_at` |

- **Relationships**: Users reference multiple `roles` by `ObjectId`; roles reference multiple `permissions` by `ObjectId`. Effective permissions are computed dynamically as the union of permissions across all assigned roles.
- **Seeded System Roles**: `Super Administrator`, `Administrator`, `Moderator`, `User`.
- **Seeded System Permissions**: `users.{read,create,update,delete,assign_roles,manage_self_roles}`, `roles.{read,create,update,delete}`, `permissions.{read,create,update,delete}`, `profile.{read,update}`.

See [backend/migrations/README.md](backend/migrations/README.md) for indexing and replica set details.

---

## 4. Authentication, JWT Lifecycle & RBAC Security

1. **Password Security**: Argon2id (`Algorithm::Argon2id`, `V0x13`, 19 MiB, 2 iterations) executed off the Actix event loop via `tokio::task::spawn_blocking`.
2. **Access JWTs**: Asymmetric **EdDSA (Ed25519)** signed tokens (default 15m TTL) validating signature, `iss`, `aud`, `exp`, `nbf`, `token_type == "access"`, and `tv` (user `token_version`). Permissions are **never** trusted blindly from JWT claims; they are resolved via a short-TTL `PermissionCache` that is explicitly invalidated immediately upon any user role, role permission, or permission change.
3. **Refresh Token Rotation & Reuse Detection**: Opaque 32-byte random tokens stored as SHA-256 hashes in MongoDB, transmitted via `HttpOnly; SameSite=Lax` cookies (`Secure` in production). Each refresh atomically rotates the token within a `family_id` (using MongoDB transactions when on a replica set). Presenting an already-rotated token triggers **reuse detection**, revoking the entire token family immediately.
4. **CSRF Protection**: Double-submit cookie pattern (`csrf_token` cookie + `X-CSRF-Token` header verified with constant-time comparison via `subtle`).
5. **Privilege Escalation Defenses** (enforced in both middleware and services):
   - Registration strictly assigns only the `User` role and rejects unexpected fields (`#[serde(deny_unknown_fields)]`).
   - Users cannot modify their own account status, delete themselves, or modify their own roles unless explicitly holding `users.manage_self_roles`.
   - Non-Super-Administrators cannot assign `Super Administrator` or any role containing permissions they do not personally hold.
   - The last active `Super Administrator` cannot be disabled, locked, demoted, or deleted.
   - Protected system roles and system permissions cannot be deleted or renamed.
6. **Frontend Session & Access UX**:
   - Access tokens are stored strictly in JS memory (never `localStorage`); concurrent 401 responses coalesce into a single `/api/v1/auth/refresh` call.
   - Signing out clears previous route history so a subsequent login as another user lands cleanly on `/` instead of hitting a stale restricted route (`403`).
   - The **Account Access Summary** card on `/profile` is hidden from standard `User` accounts and shown only to users holding `roles.read` or `permissions.read`.

---

## 5. REST API Reference (`/api/v1`)

Interactive OpenAPI 3.1 Swagger UI is available at:
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
| `POST` | `/api/v1/auth/logout` | Refresh cookie + CSRF | Revoke token family & clear cookies |
| `GET` | `/api/v1/auth/me` | Bearer JWT | Get authenticated user & effective permissions |

### Users, Roles, Permissions, Profile & Dashboard
| Method | Endpoint | Required Permission |
| --- | --- | --- |
| `GET` | `/api/v1/dashboard/stats` | Authenticated (response filtered per permission) |
| `GET` / `PUT` | `/api/v1/profile` | `profile.read` / `profile.update` |
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

Copy [backend/.env.example](backend/.env.example) and [frontend/.env.example](frontend/.env.example) for local overrides. Never commit `.env` files or PEM keys.

| Variable | Default / Example | Description |
| --- | --- | --- |
| `APP_ENV` | `development` | `development` or `production` (production requires PEM keys & secure cookies) |
| `SERVER_HOST` / `SERVER_PORT` | `127.0.0.1` / `8080` | Backend HTTP bind address |
| `MONGODB_URI` | `mongodb://localhost:27017/?replicaSet=rs0` | MongoDB connection URI |
| `MONGODB_DATABASE` | `user_management` | Target MongoDB database name |
| `JWT_ISSUER` / `JWT_AUDIENCE` | `user-management-api` / `user-management-client` | Validated JWT `iss` and `aud` claims |
| `JWT_ACCESS_TOKEN_TTL` / `JWT_REFRESH_TOKEN_TTL` | `900` / `604800` | Access (15m) and refresh (7d) lifetimes in seconds |
| `JWT_PRIVATE_KEY_PATH` / `JWT_PUBLIC_KEY_PATH` | *(ephemeral in dev)* | Ed25519 PKCS#8 PEM key paths (mandatory in production) |
| `FRONTEND_URL` / `ALLOWED_ORIGINS` | `http://localhost:5173` | Explicit CORS origins (wildcard `*` forbidden) |
| `COOKIE_SECURE` / `COOKIE_SAME_SITE` | `false` / `Lax` | Auth cookie security attributes (`COOKIE_SECURE=true` required in prod) |
| `BOOTSTRAP_SUPER_ADMIN_EMAIL` / `_PASSWORD` | `admin@example.com` / `ChangeMe123!` | Optional initial Super Admin seed if none exists |
| `VITE_API_BASE_URL` *(frontend)* | `http://localhost:8080/api/v1` | REST API base URL for Vite frontend |

---

## 7. Local Development & Docker Setup

### Option A: Docker Compose (Recommended)

```bash
docker compose up --build
```

- Frontend: `http://localhost:5173`
- Backend API: `http://localhost:8080/api/v1`
- Swagger UI: `http://localhost:8080/swagger-ui/`
- MongoDB: `mongo:latest` initialized automatically as replica set `rs0`
- Bootstrap Super Admin (dev compose defaults): `admin@example.com` / `ChangeMe123!`

### Option B: Manual Local Setup

1. Start MongoDB (`rs0` replica set recommended, or `docker compose up -d mongodb`).
2. Configure & run the backend (see [backend/README.md](backend/README.md)):
   ```bash
   cd backend
   cp .env.example .env
   cargo run
   ```
3. Configure & run the frontend (see [frontend/README.md](frontend/README.md)):
   ```bash
   cd frontend
   cp .env.example .env
   npm install
   npm run dev
   ```

---

## 8. Testing, Quality Checks & CI

### Backend (Rust 1.100.0)

```bash
cd backend
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked   # Optimized fat-LTO, stripped binary

# Optional: run live MongoDB repository, index & seeding integration test
MONGODB_TEST_URI=mongodb://localhost:27017 cargo test --test mongodb_integration_tests -- --nocapture
```

### Frontend (React 19 / Vite 8 / Vitest 5)

```bash
cd frontend
npm run format:check
npm run lint -- --max-warnings=0
npm test
npm run build
```

### GitHub Actions CI

Two automated workflows run on pushes to `main` / `develop` and on pull requests (`opened`, `synchronize`, `reopened`):
- [.github/workflows/backend-ci.yml](.github/workflows/backend-ci.yml): Enforces zero `mod.rs` files, `cargo fmt`, `cargo clippy -- -D warnings`, and runs the full test suite against a live `mongo:latest` service container.
- [.github/workflows/frontend-ci.yml](.github/workflows/frontend-ci.yml): Enforces JavaScript-only & zero TanStack dependencies, Prettier, ESLint (`--max-warnings=0`), Vitest, and Vite production builds.

See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for production hardening instructions.
