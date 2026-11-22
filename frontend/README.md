# CodeDead IAM — Frontend (React / JavaScript / Mantine UI)

Administration web application for **CodeDead IAM**, built with **React 19**, **JavaScript (ES6+)**, **React Router 7**, **Mantine UI 9**, **Vite 8** (**Node.js 26**), and **Axios** (no TypeScript and no TanStack dependencies).

## Structure

- `public/robots.txt` — Disallows search engine indexing of the IAM portal (copied into the Vite production bundle and Nginx container image).
- `src/services/` — Axios API client (`api.js` with in-memory access token, `withCredentials`, `X-CSRF-Token`, `15s` timeout, non-JSON/offline `503` detection, concurrent `401` refresh coalescing, and cross-tab refresh locking / logout sync via `navigator.locks` + `BroadcastChannel`) plus domain API modules (`auditService`, `authService`, `userService`, `roleService`, `permissionService`, `profileService`, `dashboardService`).
- `src/contexts/` & `src/hooks/` — `AuthContext` / `useAuth` and `PermissionContext` / `usePermissions` (`hasPermission`, `hasAnyPermission`, `hasAllPermissions`).
- `src/routes/` — `AppRoutes` and `ProtectedRoute` (session restoration, permission gating, `<main>` landmark support, and post-logout redirect reset).
- `src/components/` — Reusable UI primitives (`PermissionGuard`, `PageHeader`, `StatusBadge`, `ErrorAlert`), `SidebarNav`, and grouped `PermissionMatrix`.
- `src/layouts/` — `AuthLayout` and `DashboardLayout` with semantic landmarks and automatic OS color scheme detection (`defaultColorScheme="auto"`) plus manual theme toggle.
- `src/features/` — Feature pages, charts, and modals for `audit`, `auth`, `dashboard` (`DashboardPage` + zero-dependency CSP-safe `DashboardCharts`: 7-day stacked SVG activity chart, user status donut, role & permission breakdowns, dismissible 24h token-reuse alert banner, and recent audit activity feed — all strictly permission-gated), `profile` (personal details, password change, and password-confirmed self-service account deletion), `users`, `roles`, and `permissions`.
- `src/utils/` — `errors.js` (normalizes wrapped, unwrapped, and stringified API errors as well as timeout/gateway/offline failures for inline `ErrorAlert` banners and error notifications), `cookies.js`, and `formatters.js`.
- `src/test/` — Vitest 5 + React Testing Library test suites.
- `nginx.conf` & `security-headers.inc` — Non-root unprivileged Nginx configuration (port `8080`) with tightened CSP (`frame-ancestors 'none'`, `base-uri 'self'`, `form-action 'self'`, `object-src 'none'`), `Permissions-Policy`, consistent security headers across all locations, `/healthz` probe, `/api/` HTTP `503` JSON fallback (preventing `200 OK` HTML responses when the backend is offline), and SPA history fallback.

## Quick Start

```bash
cp .env.example .env
npm install
npm run dev
```

Open `http://localhost:5173`. Ensure the backend API is running on `VITE_API_BASE_URL` (default `http://localhost:8080/api/v1`; baked in at Vite build time, defaulting to `/api/v1` in `.github/workflows/docker-publish.yml` for same-origin reverse-proxy deployments).

## Commands

```bash
npm run format:check
npm run lint -- --max-warnings=0
npm test
npm run build
```

See the root [README.md](../README.md) and [../docs/DEPLOYMENT.md](../docs/DEPLOYMENT.md) for full-stack setup, Docker Compose, and Swarm/Portainer deployment instructions.
