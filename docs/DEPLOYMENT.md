# Production Deployment Guide

This guide covers hardening, key management, database topology, optimized release builds, networking, and operations for deploying **CodeDead IAM** in production.

---

## 1. Ed25519 JWT Key Management

Production mode (`APP_ENV=production`) refuses to start unless both asymmetric Ed25519 key paths are provided (partial single-key configuration is rejected in all environments, and startup runs an EdDSA sign+verify self-check to verify both PEM files match).

Generate a PKCS#8 Ed25519 key pair:

```bash
openssl genpkey -algorithm ED25519 -out /etc/codedead-iam/jwt_private.pem
openssl pkey -in /etc/codedead-iam/jwt_private.pem -pubout -out /etc/codedead-iam/jwt_public.pem
chmod 600 /etc/codedead-iam/jwt_private.pem
chmod 644 /etc/codedead-iam/jwt_public.pem
```

Configure the backend:

```env
APP_ENV=production
JWT_PRIVATE_KEY_PATH=/etc/codedead-iam/jwt_private.pem
JWT_PUBLIC_KEY_PATH=/etc/codedead-iam/jwt_public.pem
JWT_ISSUER=https://api.example.com
JWT_AUDIENCE=https://admin.example.com
JWT_ACCESS_TOKEN_TTL=900
JWT_REFRESH_TOKEN_TTL=604800
REVOKED_TOKEN_RETENTION_SECONDS=3600
```

Never bake PEM keys into Docker images or commit them to source control. Mount them via Kubernetes Secrets, Vault, or AWS/GCP Secret Manager.

---

## 2. MongoDB Replica Set & Security

Multi-document ACID transactions (used for atomic refresh-token rotation) require a MongoDB Replica Set (minimum 3 nodes recommended in production: 1 primary + 2 secondaries).

Checklist:
- Pin an explicit MongoDB major release in production and follow MongoDB's sequential `featureCompatibilityVersion` upgrade procedure (`docker-compose.yml` pins `mongo:9` bound to `127.0.0.1:27017` for local development).
- Enable MongoDB authentication (`--auth`) and SCRAM-SHA-256 credentials.
- Enable TLS (`tls=true`) on the connection string.
- Restrict network access to backend subnets only.
- Use automated daily backups with point-in-time recovery (oplog).
- Verify startup logs confirm: `MongoDB replica set / sharded cluster detected: multi-document transactions enabled`.

Example production URI and database configuration (`MONGODB_DATABASE` can be set to any database name; ensure your MongoDB user has `readWrite` permissions on that database; you may alternatively mount a Docker/Kubernetes secret and set `MONGODB_URI_FILE=/run/secrets/mongodb_uri`):

```env
MONGODB_URI=mongodb+srv://app_user:STRONG_SECRET@cluster0.example.mongodb.net/?retryWrites=true&w=majority&tls=true
MONGODB_DATABASE=codedead_iam
MONGODB_MAX_POOL_SIZE=50
MONGODB_MIN_POOL_SIZE=5
```

---

## 3. Optimized Release Builds

`backend/Cargo.toml` configures `[profile.release]` for maximum runtime performance and minimal binary footprint (`opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`, `panic = "abort"`, `incremental = false`).

- `backend/Dockerfile` compiles with `cargo build --release --locked` using Rust `1.100.0` (with `backend/.dockerignore` excluding local secrets/artifacts), installs `/app/auth-backend` and `/app/config` read-only as `root:root` inside a non-root `debian:bookworm-slim` runtime image (`appuser` UID/GID `10001`), and health-checks `/health/ready`.
- `frontend/Dockerfile` compiles the Vite 8 production bundle with `node:26-alpine` (`npm ci && npm run build`, including `public/robots.txt`, with `frontend/.dockerignore`), renders `frontend/security-headers.inc` at build time so CSP defaults to `connect-src 'self'` (auto-appending only an explicit cross-origin `VITE_API_BASE_URL` origin), and serves static assets via non-root `nginxinc/nginx-unprivileged:stable-alpine` on port `8080` (`server_tokens off`, `Cache-Control: no-cache` on `/index.html`, `/api/` `503` JSON fallback when the backend is offline, and SPA history fallback).

---

## 4. HTTPS, Cookies, CORS & CSRF

Terminate TLS at a reverse proxy (Nginx, Envoy, Traefik, or Cloud Load Balancer) and set:

```env
FRONTEND_URL=https://admin.example.com
ALLOWED_ORIGINS=https://admin.example.com
COOKIE_SECURE=true
COOKIE_SAME_SITE=Lax
COOKIE_DOMAIN=.example.com
```

Notes:
- Wildcard CORS (`*`) is forbidden by startup validation when credentials are enabled. Setting `FRONTEND_URL` automatically replaces the dev `http://localhost:5173` default origin unless `ALLOWED_ORIGINS` overrides it.
- `COOKIE_SAME_SITE` is validated at startup (`Lax`, `Strict`, or `None`); if frontend and API live on different top-level domains, set `COOKIE_SAME_SITE=None` with `COOKIE_SECURE=true` (`SameSite=None` with `COOKIE_SECURE=false` is rejected).
- Refresh tokens are stored in `HttpOnly; Secure` cookies scoped to `/api/v1/auth` and protected against CSRF via double-submit (`csrf_token` cookie + `X-CSRF-Token` header).

---

## 5. Rate Limiting & Reverse Proxy Headers

Configure the reverse proxy to forward `CF-Connecting-IP` (when behind Cloudflare / `cloudflared`) or overwrite `X-Forwarded-For` / `X-Real-IP` safely from trusted proxies only, and tune rate limits (`TRUST_PROXY_HEADERS=true` checks `CF-Connecting-IP` first, then `X-Forwarded-For` / `X-Real-IP`):

```env
RATE_LIMIT_ENABLED=true
RATE_LIMIT_AUTH_PER_MINUTE=20
RATE_LIMIT_API_PER_MINUTE=300
TRUST_PROXY_HEADERS=true
```

For horizontally scaled multi-replica backend deployments, place an edge/gateway rate limiter (Envoy, Kong, Cloudflare, or Redis-backed rate limiting) in front of the API pods.

Verify effective security flags in the startup log line `Starting HTTP server`, which prints `environment`, `trust_proxy_headers`, `rate_limit_enabled`, and `swagger_enabled`. (Boolean environment variables accept case-insensitive `true`/`false`, `1`/`0`, or `yes`/`no`.)

---

## 6. Health Checks & Observability

- **Liveness probe:** `GET /health` (returns HTTP 200 when the HTTP worker loop is responsive).
- **Readiness probe:** `GET /health/ready` (pings MongoDB and returns HTTP 200 when ready, or HTTP 503 `DATABASE_UNAVAILABLE` if unreachable; keep probes internal to Swarm/orchestrator healthchecks rather than exposed publicly).
- **Structured & audit logging:** Control verbosity via `RUST_LOG=info,auth_backend=info`. Every response emits an `X-Request-Id` header for end-to-end log correlation. User, organisation, role, permission, profile, and authentication events emit structured `auth_backend::audit` logs to stdout and are persisted in the MongoDB `audit_logs` collection (viewable via `GET /api/v1/audit-logs`, the `/audit-logs` UI, and the permission-gated Dashboard's 7-day activity trends & 24h failed-login / token-reuse alerts with `audit_logs.read`).
- **OpenAPI / Swagger UI:** Disabled by default when `APP_ENV=production` (set `ENABLE_SWAGGER_UI=true` only if you intentionally expose schema docs behind proxy authentication).
- **Audit Log Retention:** Configured via `AUDIT_LOG_RETENTION_DAYS` (defaults to `90` days; `0` drops/disables the `audit_created_at_ttl` index).
- **Refresh Token Cleanup:** Configured via `REVOKED_TOKEN_RETENTION_SECONDS` (defaults to `3600` seconds / 1 hour; `0` drops/disables the `revoked_at_ttl` index). Rotation also prunes each session family to at most 5 recent revoked tokens, while logout and user/account deletion hard-delete refresh tokens immediately.

---

## 7. Initial Super Administrator Bootstrap

Set `BOOTSTRAP_SUPER_ADMIN_EMAIL` and a strong `BOOTSTRAP_SUPER_ADMIN_PASSWORD` (or `BOOTSTRAP_SUPER_ADMIN_PASSWORD_FILE` via a secret mount, plus optional `BOOTSTRAP_SUPER_ADMIN_FIRST_NAME` / `LAST_NAME`) only for initial provisioning. Once created, remove `BOOTSTRAP_SUPER_ADMIN_PASSWORD` from the runtime environment and rotate the administrator password via `PATCH /api/v1/profile/password`.

---

## 8. Docker Swarm & Portainer Deployment (with MongoDB Atlas)

Ready-to-use deployment templates are included in `deploy/`:
- [`deploy/swarm-stack.yml`](../deploy/swarm-stack.yml) — Swarm stack with Ed25519 Docker Secrets (`/run/secrets/jwt_{private,public}.pem` owned by UID `10001`), zero-downtime `start-first` rollouts, `/health/ready` checks, and same-domain Traefik routing (`/api` → backend, `/` → frontend).
- [`deploy/portainer.env.example`](../deploy/portainer.env.example) — Environment variable template for Portainer's **Load variables from `.env` file** button.
- [`.github/workflows/docker-publish.yml`](../.github/workflows/docker-publish.yml) — Verifies backend/frontend lint & tests and publishes both container images to `ghcr.io` on pushes to `main`, `v*.*.*` tags, or manual `workflow_dispatch` (defaults `VITE_API_BASE_URL=/api/v1` with `connect-src 'self'` CSP for same-origin routing; if overridden to an explicit cross-origin URL such as `https://api.example.com/api/v1`, the frontend `Dockerfile` automatically appends that origin to Nginx's CSP `connect-src`).

### Quick Steps in Portainer
1. Create two external secrets in **Portainer → Secrets**: `codedead_jwt_private_pem` and `codedead_jwt_public_pem`.
2. Whitelist your Swarm node public egress IP(s) in **MongoDB Atlas → Network Access**.
3. In **Portainer → Stacks → Add stack**, paste `deploy/swarm-stack.yml`, populate the variables from `deploy/portainer.env.example`, and click **Deploy the stack**.
4. Note: Keep `replicas: 1` for Atlas M0 free tier (500-connection limit). If scaling backend `replicas > 1`, set `PERMISSION_CACHE_TTL_SECONDS=0` so permission revocations propagate immediately across replicas.
