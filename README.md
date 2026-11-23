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
                                                                         │    organisations, refresh_tokens,  │
                                                                         │    audit_logs                      │
                                                                         └────────────────────────────────────┘
```

### Layered Backend Separation

- **Controllers (`web/controller/*.rs`)**: Thin HTTP adapters; extract DTOs/params, attach cookies, call services, and format `ApiResponse`. Never access MongoDB or embed business logic.
- **DTOs (`web/dto/*.rs`)**: Strict separation between MongoDB BSON DAOs and API request/response contracts (`#[serde(deny_unknown_fields)]` blocks mass assignment; password hashes are never serialized).
- **Services (`services/*.rs`)**: Business rules, DTO validation, defense-in-depth permission & privilege-escalation checks, structured `tracing` + persistent MongoDB audit logging (`auth_backend::audit`), permission-filtered dashboard analytics & aggregations, cache invalidation, and session revocation.
- **Repositories & DAOs (`repository/*/*_repository.rs`, `repository/*/dao/*.rs`)**: Trait-based MongoDB persistence with explicit indexes, transaction support, and BSON data access objects.
- **Module Convention**: Modern Rust 2024 directory modules — **zero `mod.rs` files** exist anywhere in the project.
- **Safety Lints & Release Optimization**: `unsafe_code = "forbid"`, `clippy::{unwrap_used, expect_used, panic} = "deny"`, and an optimized `[profile.release]` (`opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`, `panic = "abort"`).

### Frontend Highlights

- **Pure JavaScript (ES6+)** with React 19, React Router 7, Mantine UI 9, Vite 8, and Axios — no TypeScript and no TanStack packages.
- **Secure Token Lifecycle**: Access tokens live exclusively in JS memory (never `localStorage`), refresh tokens live in `HttpOnly` cookies, concurrent `401` responses coalesce into a single `/api/v1/auth/refresh` request, and open browser tabs coordinate token refresh & logout via `navigator.locks` + `BroadcastChannel` to prevent multi-tab refresh-token reuse races.
- **RBAC UX & Security Dashboard**: Navigation links, routes, Organisations (`/organisations`), Audit Logs (`/audit-logs`), profile Account Access Summary, self-service Personal & Organisation account registration, and the interactive Dashboard (KPI cards, 7-day stacked SVG security/audit activity chart, user status donut chart, top organisations, role & permission distribution bars, dismissible 24h token-reuse alert banner, recent audit feed, active sessions, and user organisation badges) are strictly guarded by user permissions both server-side and client-side; logging out clears route history state so a subsequent user never inherits a restricted redirect.
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
│   ├── .dockerignore
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
│   │   ├── web/controller.rs + web/controller/{audit,auth,users,roles,permissions,organisations,profile,dashboard,health}.rs
│   │   ├── web/dto.rs + web/dto/{audit,auth,users,roles,permissions,organisations,profile,dashboard,health}.rs
│   │   ├── services.rs + services/{audit,auth,users,roles,permissions,organisations,profile,dashboard}.rs
│   │   ├── repository.rs
│   │   ├── repository/user.rs + repository/user/{user_repository.rs, dao.rs, dao/user.rs}
│   │   ├── repository/role.rs + repository/role/{role_repository.rs, dao.rs, dao/role.rs}
│   │   ├── repository/permission.rs + repository/permission/{permission_repository.rs, dao.rs, dao/permission.rs}
│   │   ├── repository/organisation.rs + repository/organisation/{organisation_repository.rs, dao.rs, dao/organisation.rs}
│   │   ├── repository/auth.rs + repository/auth/{refresh_token_repository.rs, dao.rs, dao/refresh_token.rs}
│   │   ├── repository/audit.rs + repository/audit/{audit_log_repository.rs, dao.rs, dao/audit_log.rs}
│   │   └── test_support.rs
│   └── tests/
│       ├── audit_log_tests.rs
│       ├── auth_tests.rs
│       ├── dashboard_tests.rs
│       ├── organisation_tests.rs
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
    ├── .dockerignore
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
        ├── contexts/{contextInstances.js,AuthContext.jsx,PermissionContext.jsx}
        ├── hooks/{useAuth,usePermissions}.js
        ├── services/{api,auditService,authService,userService,roleService,permissionService,organisationService,profileService,dashboardService}.js
        ├── components/{common,navigation,forms}/
        ├── layouts/{AuthLayout,DashboardLayout}.jsx
        ├── routes/{AppRoutes,ProtectedRoute}.jsx
        ├── features/{audit,auth,dashboard,organisations,profile,users,roles,permissions}/
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
| `organisations` | `_id`, `name`, `members: [{ user_id, role, status, joined_at }]`, `version`, `created_at`, `updated_at` | Case-insensitive unique `name` (`org_name_unique`, `en` strength-2 collation), multikey `members.user_id` (`org_members_user_id_idx`), `created_at` (`org_created_at_desc`) |
| `refresh_tokens` | `_id`, `user_id`, `token_hash` (SHA-256), `family_id`, `expires_at`, `revoked_at`, `replaced_by_hash`, `user_agent`, `ip_address`, `created_at` | Unique `token_hash` (`token_hash_unique`), `user_id` (`user_id_idx`), `family_id` (`family_id_idx`), `(family_id, revoked_at)` (`family_revoked_at_idx`), TTL on `expires_at` (`expires_at_ttl`), TTL on `revoked_at` (`revoked_at_ttl`) |
| `audit_logs` | `_id`, `action`, `resource_type`, `resource_id`, `resource_name`, `actor_id`, `actor_email`, `summary`, `details`, `created_at` | `created_at` (`audit_created_at_desc`), TTL on `created_at` (`audit_created_at_ttl`, configurable via `AUDIT_LOG_RETENTION_DAYS`), `(action, created_at)` (`audit_action_created_at`), `(resource_type, created_at)` (`audit_resource_type_created_at`), `(actor_id, created_at)` (`audit_actor_id_created_at`) |

- **Relationships**: Users reference multiple `roles` by `ObjectId`; roles reference multiple `permissions` by `ObjectId`; organisations embed `members` (`{ user_id, role, status, joined_at }` with org-scoped roles `owner`/`admin`/`member` and statuses `active`/`invited`/`suspended`). A single user can belong to multiple organisations simultaneously. Effective platform permissions are computed dynamically as the union of permissions across all assigned roles.
- **Seeded System Roles**: `Super Administrator`, `Administrator`, `Moderator`, `User`.
- **Seeded System Permissions**: `users.{read,create,update,delete,assign_roles,manage_self_roles}`, `roles.{read,create,update,delete}`, `permissions.{read,create,update,delete}`, `organisations.{read,create,update,delete,manage_members}`, `audit_logs.read`, `profile.{read,update,delete}` (newly-introduced system permissions are automatically backfilled onto existing system roles on startup).

See [backend/migrations/README.md](backend/migrations/README.md) for indexing and replica-set transaction details.

---

## 4. Authentication, JWT Lifecycle & RBAC Security

1. **Password Security & Timing Protection**: Argon2id (`argon2 0.6`, `Algorithm::Argon2id`, `V0x13`, 19 MiB, 2 iterations) executed off the Actix event loop via `tokio::task::spawn_blocking`. Unknown-email logins execute a dummy Argon2id verification to prevent timing-based user enumeration, all failed login attempts are recorded as `auth.login_failed` audit events, and post-Argon2id updates (`login` `last_login_at` and `change_password`) use atomic MongoDB `$set`/`$inc` filters on `status: "active"` so concurrent admin disables or role revocations can never be overwritten.
2. **Access JWTs**: Asymmetric **EdDSA (Ed25519)** signed tokens (`jsonwebtoken 12`, `ed25519-dalek 3`, default 15m TTL, with a startup keypair sign+verify self-check) validating signature, `iss`, `aud`, `exp`, `nbf`, `token_type == "access"`, and `tv` (user `token_version`, incremented immediately upon role changes, account deactivation, or password changes). Permissions are **never** trusted blindly from JWT claims; they are resolved via a bounded, generation-tracked `PermissionCache` explicitly invalidated immediately upon any user role, role permission, or permission mutation.
3. **Refresh Token Rotation, Reuse Detection & Cleanup**: Opaque 32-byte random tokens stored as SHA-256 hashes in MongoDB, transmitted via `HttpOnly; SameSite=Lax` cookies (`Secure` in production). Each refresh atomically rotates the token within a `family_id` (using MongoDB transactions on replica sets, with `WriteConflict` / `TransientTransactionError` handled cleanly as concurrent reuse) and prunes older revoked tokens in that family to at most **5** recent entries so repeated page reloads cannot grow the collection unboundedly. Presenting an already-rotated token triggers **reuse detection**, revoking the entire token family immediately (and the frontend coordinates multi-tab refreshes, login/register broadcasts, late-401 token reuse, and logout sync via `navigator.locks` + `BroadcastChannel` to prevent benign cross-tab races). Revoked tokens are automatically purged by MongoDB's `revoked_at` TTL index (`REVOKED_TOKEN_RETENTION_SECONDS`, default `3600` seconds; `0` drops the index), expired tokens are purged via the `expires_at` TTL index, and logout or account/user deletion hard-deletes session tokens immediately.
4. **CSRF Protection**: Double-submit cookie pattern (`csrf_token` cookie + `X-CSRF-Token` header verified with constant-time comparison via `subtle`).
5. **Privilege Escalation Defenses** (enforced in both HTTP middleware and services):
   - Registration strictly assigns only the `User` role, rejects unexpected fields (`#[serde(deny_unknown_fields)]`), and withholds tokens/cookies when `REQUIRE_EMAIL_VERIFICATION=true`.
   - Users cannot modify their own account status, delete themselves via administration endpoints, or modify their own roles unless explicitly holding `users.manage_self_roles`. Self-service account deletion (`DELETE /api/v1/profile`, requiring `profile.delete`) requires current password confirmation, revokes all sessions, and still protects the last active `Super Administrator`.
   - Non-Super-Administrators cannot assign `Super Administrator` or any role containing permissions they do not personally hold, and cannot edit, disable, delete, or change roles of any user account that holds permissions they do not personally hold (e.g., a `Moderator` cannot modify or disable an `Administrator`).
   - The last active `Super Administrator` cannot be disabled, locked, demoted, or deleted.
   - Protected system roles and system permissions cannot be deleted or renamed.
6. **Organisation Isolation & User Data Minimization**:
   - Users can belong to multiple organisations simultaneously with independent org-scoped roles (`owner`, `admin`, `member`) and statuses (`active`, `invited`, `suspended`). Organisation mutations use optimistic concurrency (`version` field) to prevent lost updates.
   - Any authenticated user can create a new organisation (`POST /api/v1/organisations`, becoming its active `owner`, capped at 10 self-service owned organisations and unable to omit themselves or attach arbitrary `initial_member_ids` / `initial_owner_email`). Platform admins creating an organisation with `include_creator: false` can delegate ownership via `initial_owner_email` (or `initial_member_ids`) to an active user account, ensuring every organisation starts with at least one active owner.
   - Organisation owners/admins can invite active users strictly by their signup email address (never by raw `user_id`, capped at 100 pending invitations per organisation, with non-active target accounts returning the same `404 USER_NOT_FOUND` so account status is never leaked), cancel pending invitations, remove members, and rename or delete their own organisation (owner only), while remaining strictly isolated from all other organisations (`403 Forbidden`).
   - Self-service organisation invitations start in `invited` status and require the user to explicitly accept (`POST /api/v1/organisations/{id}/accept`) before becoming `active` (declining via `POST /api/v1/organisations/{id}/leave` records a distinct `organisation.invitation_declined` audit event). Self-service managers cannot promote non-active (`invited`/`suspended`) members to `owner`, cannot change the status of a pending invitation (preventing both direct activation and `Invited -> Suspended -> Active` bypasses) or revert active members back to `invited`, and personal names (`first_name`/`last_name`) remain redacted while `invited` for callers without global `users.read` (once accepted, names stay visible even if later suspended).
   - Organisations never receive access to user profiles (`phone`, `bio`, `job_title`, `department`), global RBAC roles/permissions, login timestamps, or memberships in other organisations. Regular org `member`s cannot view other members' rosters, and the last active `owner` is protected (`409 LAST_ORG_OWNER_PROTECTED`) across role changes, removals, leaving, and account deletion (while solo organisations with no other active members are automatically deleted only when their active owner's account is deleted, preventing empty/ownerless orphan organisations).

---

## 5. REST API Reference (`/api/v1`)

Interactive OpenAPI 3.1 Swagger UI (`utoipa 6` + vendored `utoipa-swagger-ui 10`) is enabled by default in development and disabled by default in production (`ENABLE_SWAGGER_UI`):
- `GET /swagger-ui/`
- `GET /api/v1/openapi.json`

### Health
- `GET /health` — Liveness probe (`200 OK`)
- `GET /health/ready` — Readiness probe (verifies MongoDB connectivity; returns `200 OK` or `503 Service Unavailable`)

### Authentication
| Method | Endpoint | Auth / Permission | Description |
| --- | --- | --- | --- |
| `POST` | `/api/v1/auth/register` | Public | Register a personal `User` account (or an Organisation account when `organisation_name` is provided) |
| `POST` | `/api/v1/auth/login` | Public | Authenticate & issue access JWT + refresh cookie |
| `POST` | `/api/v1/auth/refresh` | Refresh cookie + CSRF | Rotate refresh token & issue new access JWT |
| `POST` | `/api/v1/auth/logout` | Refresh cookie + CSRF | Delete token family & clear cookies |
| `GET` | `/api/v1/auth/me` | Bearer JWT | Get authenticated user & effective permissions |

### Users, Organisations, Roles, Permissions, Audit Logs, Profile & Dashboard
| Method | Endpoint | Required Permission |
| --- | --- | --- |
| `GET` | `/api/v1/dashboard/stats` | Authenticated (strictly filtered per permission: `users.read`, `organisations.read`, `roles.read`, `permissions.read`, `audit_logs.read`; always includes caller's own `my_active_sessions` and `my_organisations`) |
| `GET` / `PUT` / `DELETE` | `/api/v1/profile` | `profile.read` / `profile.update` / `profile.delete` |
| `PATCH` | `/api/v1/profile/password` | `profile.update` |
| `GET` / `POST` | `/api/v1/users` | `users.read` / `users.create` |
| `GET` / `PUT` / `DELETE` | `/api/v1/users/{id}` | `users.read` / `users.update` / `users.delete` |
| `PATCH` | `/api/v1/users/{id}/status` | `users.update` |
| `GET` / `PUT` | `/api/v1/users/{id}/roles` | `users.read` / `users.assign_roles` |
| `GET` | `/api/v1/users/{id}/permissions` | `users.read` |
| `GET` | `/api/v1/users/{id}/organisations` | `users.read` + `organisations.read` |
| `GET` | `/api/v1/organisations/me` | Authenticated (caller's own organisation memberships & pending invitations) |
| `GET` / `POST` | `/api/v1/organisations` | `organisations.read` (GET) / Authenticated (POST — self-service users become active `owner`, capped at 10 owned orgs) |
| `GET` / `PUT` / `DELETE` | `/api/v1/organisations/{id}` | `organisations.read` or org `owner`/`admin` (GET) / `organisations.update` or org `owner` (PUT) / `organisations.delete` or org `owner` (DELETE) |
| `POST` | `/api/v1/organisations/{id}/members` | `organisations.manage_members` or org `owner`/`admin` (self-service invites by signup email only) |
| `PATCH` / `DELETE` | `/api/v1/organisations/{id}/members/{user_id}` | `organisations.manage_members` or org `owner`/`admin` |
| `POST` | `/api/v1/organisations/{id}/accept` | Authenticated (accept pending invitation to organisation) |
| `POST` | `/api/v1/organisations/{id}/leave` | Authenticated (decline invitation or leave organisation) |
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
| `MONGODB_URI` / `MONGODB_URI_FILE` | `mongodb://localhost:27017` | MongoDB connection URI (standalone, replica set, or Atlas `mongodb+srv://`; supports Docker Secret `_FILE` path) |
| `MONGODB_DATABASE` | `user_management` | Configurable target MongoDB database name (override via `.env`, `docker-compose.yml`, or Portainer stack env) |
| `MONGODB_MAX_POOL_SIZE` / `MONGODB_MIN_POOL_SIZE` | `50` / `5` | MongoDB driver connection pool bounds |
| `JWT_ISSUER` / `JWT_AUDIENCE` | `user-management-api` / `user-management-client` | JWT `iss` and `aud` validation claims |
| `JWT_ACCESS_TOKEN_TTL` / `JWT_REFRESH_TOKEN_TTL` | `900` / `604800` | Token lifetimes in seconds (15 min / 7 days) |
| `REVOKED_TOKEN_RETENTION_SECONDS` | `3600` | MongoDB TTL retention in seconds for revoked refresh tokens (1 hour; `0` drops/disables `revoked_at_ttl`); rotation also prunes to at most 5 revoked tokens per session family |
| `JWT_PRIVATE_KEY_PATH` / `JWT_PUBLIC_KEY_PATH` | *(empty in dev)* | Ed25519 PKCS#8/SPKI PEM key paths (both required together; mandatory in production; sign+verify checked at startup) |
| `FRONTEND_URL` / `ALLOWED_ORIGINS` | `http://localhost:5173` | Explicit CORS origin allowlist (setting `FRONTEND_URL` replaces the dev default; wildcards forbidden) |
| `COOKIE_SECURE` / `COOKIE_SAME_SITE` / `COOKIE_DOMAIN` | `false` / `Lax` / *(empty)* | Refresh & CSRF cookie attributes (`COOKIE_SAME_SITE` validated at startup; `None` requires `COOKIE_SECURE=true`) |
| `RATE_LIMIT_ENABLED` / `RATE_LIMIT_AUTH_PER_MINUTE` / `RATE_LIMIT_API_PER_MINUTE` | `true` / `30` / `300` | Cross-worker shared per-IP rate limiting thresholds (bounded memory with stale/oldest bucket eviction; applies to both `/api/v1` and stricter `/api/v1/auth`) |
| `TRUST_PROXY_HEADERS` | `false` | When `true`, rate limiting and audit logs prefer Cloudflare's `CF-Connecting-IP` header, falling back to `X-Forwarded-For` / `X-Real-IP` (valid IPs only); when `false`, uses only the direct socket peer IP |
| `ENABLE_SWAGGER_UI` | `true` (dev) / `false` (prod) | Controls whether `/swagger-ui/` and `/api/v1/openapi.json` are exposed (disabled by default in production) |
| `AUDIT_LOG_RETENTION_DAYS` | `90` | MongoDB TTL index retention period in days for `audit_logs` (`0` drops/disables `audit_created_at_ttl`) |
| `PERMISSION_CACHE_TTL_SECONDS` | `30` | Bounded, generation-tracked in-memory RBAC cache TTL in seconds (`0` disables caching for multi-replica deployments) |
| `REQUIRE_EMAIL_VERIFICATION` | `false` | When `true`, newly registered accounts start in `pending_verification` status without issuing session tokens/cookies |
| `BOOTSTRAP_SUPER_ADMIN_EMAIL` / `BOOTSTRAP_SUPER_ADMIN_PASSWORD` (`_FILE`) | `admin@example.com` / *(empty)* | Optional initial Super Administrator bootstrap (also supports `BOOTSTRAP_SUPER_ADMIN_FIRST_NAME` / `LAST_NAME` and `_FILE` secret path) |
| `RUST_LOG` | `info,auth_backend=debug,actix_web=info` | `tracing-subscriber` filter expression (`info` in production container) |

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

Uses `mongo:9` configured as a single-node replica set (`rs0`, bound to `127.0.0.1:27017`), the optimized Rust release container, and the Nginx frontend container:
- **Frontend**: `http://localhost:5173`
- **Backend API**: `http://localhost:8080/api/v1`
- **Swagger UI**: `http://localhost:8080/swagger-ui/`
- **Bootstrap Super Admin (dev compose defaults)**: `admin@example.com` / `ChangeMe123!`

### Option B: Manual Local Setup

1. Start MongoDB (`docker compose up -d mongodb`, or a local replica set `rs0` on `127.0.0.1:27017`; `backend/.env.example` includes `directConnection=true` so the host connects directly to the containerized replica set).
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
- **[.github/workflows/backend-ci.yml](.github/workflows/backend-ci.yml)**: Verifies zero `mod.rs` files, runs `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and executes the full unit, API/RBAC, and live `mongo:9` replica-set (`rs0`) integration test suite.
- **[.github/workflows/frontend-ci.yml](.github/workflows/frontend-ci.yml)**: Verifies JavaScript-only source and zero TanStack dependencies, runs Prettier, ESLint (`--max-warnings=0`), Vitest, and the Vite production build.
- **[.github/workflows/docker-publish.yml](.github/workflows/docker-publish.yml)**: Gates releases on backend/frontend lint, tests, and `npm audit --audit-level=high`, then builds and publishes backend and frontend container images to `ghcr.io` on pushes to `main`, `v*.*.*` tags, or manual dispatch (ready for [deploy/swarm-stack.yml](deploy/swarm-stack.yml) in Portainer).

See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for production hardening instructions.
