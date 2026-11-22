# Docker Swarm & Portainer Deployment

This directory contains production deployment templates for running **CodeDead IAM** on **Docker Swarm** via **Portainer** backed by **MongoDB Atlas** (Free M0 or higher).

## Files

- [`swarm-stack.yml`](swarm-stack.yml) — Docker Swarm stack using pre-built GHCR images (both backend and unprivileged frontend running non-root on port `8080`), external Ed25519 Docker Secrets (`/run/secrets/jwt_{private,public}.pem` owned by UID `10001`), `/health/ready` checks, `start-first` zero-downtime rollouts with rollback, production defaults (`TRUST_PROXY_HEADERS=true` supporting `CF-Connecting-IP` behind Cloudflare Tunnel + Traefik, `ENABLE_SWAGGER_UI=false`, `AUDIT_LOG_RETENTION_DAYS=90`), and Traefik same-domain routing (`/api` & `/health` → backend, `/` → frontend).
- [`portainer.env.example`](portainer.env.example) — Environment variable template for Portainer's **"Load variables from `.env` file"** option.

## Deployment Steps

1. **Publish images**: Push to `main` or tag `v*.*.*` to trigger [`.github/workflows/docker-publish.yml`](../.github/workflows/docker-publish.yml) (publishes `ghcr.io/<owner>/<repo>-backend` and `ghcr.io/<owner>/<repo>-frontend`).
2. **Generate Ed25519 keys & create Swarm secrets**:
   ```bash
   openssl genpkey -algorithm ED25519 -out jwt_private.pem
   openssl pkey -in jwt_private.pem -pubout -out jwt_public.pem
   ```
   In **Portainer → Secrets**, create external secrets named `codedead_jwt_private_pem` and `codedead_jwt_public_pem`.
3. **Configure MongoDB Atlas**: Create a database user with `readWrite` on your configured `MONGODB_DATABASE` (defaults to `user_management`) and whitelist your Swarm node public IP(s) under **Network Access**.
4. **Deploy stack in Portainer**: Go to **Stacks → Add stack**, paste `swarm-stack.yml`, populate the variables from `portainer.env.example`, and click **Deploy the stack**.
5. **Post-deploy**: Sign in with the bootstrap Super Administrator, rotate the password via **My Profile**, and remove `BOOTSTRAP_SUPER_ADMIN_PASSWORD` from the stack environment variables.

For full production hardening guidance (multi-replica `PERMISSION_CACHE_TTL_SECONDS=0`, TLS/CORS/SameSite rules, and observability), see [`docs/DEPLOYMENT.md`](../docs/DEPLOYMENT.md).
