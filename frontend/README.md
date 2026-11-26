# CodeDead IAM — Frontend (React / JavaScript / Mantine UI)

Administration web application for **CodeDead IAM**, built with **React 19**, **JavaScript (ES6+)**, **React Router 7**, **Mantine UI 9**, **Vite 8**, **Yarn Berry (`yarn@4.19.0`)** (**Node.js 26**), and **Axios** (no TypeScript and no TanStack dependencies).

## Structure

- `public/robots.txt` — Disallows search engine indexing of the IAM portal (copied into the Vite production bundle and Nginx container image).
- `src/services/` — Axios API client (`api.js` with in-memory access token, `withCredentials`, `X-CSRF-Token`, `15s` timeout, non-JSON/offline `503` detection, concurrent `401` refresh coalescing, late-401 token reuse without redundant refresh, and cross-tab refresh locking / login / logout sync via `navigator.locks` + `BroadcastChannel`) plus domain API modules (`auditService`, `authService`, `userService`, `roleService`, `permissionService`, `organisationService`, `profileService`, `dashboardService`).
- `src/contexts/` & `src/hooks/` — `AuthContext` / `useAuth` and `PermissionContext` / `usePermissions` (`hasPermission`, `hasAnyPermission`, `hasAllPermissions`).
- `src/routes/` — `AppRoutes` (with `React.lazy` route & layout code splitting, `Suspense`, and `RouteErrorBoundary` recovery alongside `vite:preloadError` reload handling in `main.jsx`) and `ProtectedRoute` (session restoration, permission gating, `<main>` landmark support, and post-logout redirect reset).
- `src/components/` — Reusable UI primitives (`PermissionGuard`, `PageHeader`, `StatusBadge`, `ErrorAlert`, `RouteFallback`, `RouteErrorBoundary`), `SidebarNav`, and grouped `PermissionMatrix`.
- `src/layouts/` — `AuthLayout` and `DashboardLayout` with nested `<Suspense>` outlet boundaries, semantic landmarks, keyboard-accessible account menu (`UnstyledButton`), `profile.read`-gated profile links, and automatic OS color scheme detection (`defaultColorScheme="auto"`) plus manual theme toggle.
- `src/features/` — Feature pages, charts, and modals with automatic pagination clamping on deletions for `audit` (including `organisation.invitation_declined` and `auth.refresh_token_transport_mismatch` filtering & warning badges), `auth` (including Personal & Organisation account registration), `dashboard` (`DashboardPage` + zero-dependency CSP-safe, keyboard-accessible chart components `ActivityTrendChart`, `UserStatusDonutChart`, `OrganisationDistributionCard`, `RoleDistributionCard`, and `ResourceBreakdownCard`: 7-day stacked SVG activity chart, user status donut, top organisations, role & permission breakdowns, per-user dismissible 24h token-reuse alert banner, pending organisation invitation accept/confirmed-decline actions, recent audit activity feed, and Quick Navigation—with RBAC access details gated to `roles.read` / `permissions.read`), `organisations` (My Organisations + All Organisations views with `?search=` query-param deep-linking and preserved row-level org-owner permissions, self-service organisation creation plus delegated `initial_owner_email` creation for platform admins, signup-email member invitations with privacy redaction while `invited`, roster status summary badges & search/status filters, invitation accept/decline/cancel/leave, org roles `owner`/`admin`/`member`, last-owner & non-active owner-promotion control protection, confirmed self-demotion/member removal with modal auto-close on self-removal, and owner-initiated rename/deletion), `profile` (`profile.update`-gated personal details & password change, plus password-confirmed self-service account deletion with a quick **Manage organisations** shortcut), `users` (including `users.manage_self_roles` gating, organisation-ownership guidance on user deletion, and `organisations.read`-gated user organisation deep-links in `UserDetailDrawer`), `roles`, and `permissions`.
- `src/utils/` — `errors.js` (normalizes wrapped, unwrapped, and stringified API errors as well as timeout/gateway/offline failures for inline `ErrorAlert` banners and error notifications), `cookies.js` (`URIError`-safe cookie decoding), `formatters.js`, and `pagination.js` (`fetchAllPages` for paginated dropdown catalogs).
- `src/test/` — Vitest 5 + React Testing Library test suites.
- `nginx.conf` & `security-headers.inc` — Non-root unprivileged Nginx configuration (port `8080`, `server_tokens off`) with dynamic build-time CSP (`connect-src 'self'`, auto-appending an explicit cross-origin `VITE_API_BASE_URL` origin when configured, plus `frame-ancestors 'none'`, `base-uri 'self'`, `form-action 'self'`, `object-src 'none'`), `Permissions-Policy`, `Cache-Control: no-cache` on `/index.html`, `/healthz` probe, `/api/` HTTP `503` JSON fallback (preventing `200 OK` HTML responses when the backend is offline), and SPA history fallback.

## Quick Start

```bash
cp .env.example .env
yarn install
yarn dev
```

Open `http://localhost:5173`. Ensure the backend API is running on `VITE_API_BASE_URL` (default `http://localhost:8080/api/v1`; baked in at Vite build time, defaulting to `/api/v1` in `.github/workflows/docker-publish.yml` for same-origin reverse-proxy deployments).

## Commands

```bash
yarn format:check
yarn lint --max-warnings=0
yarn test
yarn build
```

See the root [README.md](../README.md) and [../docs/DEPLOYMENT.md](../docs/DEPLOYMENT.md) for full-stack setup, Docker Compose, and Swarm/Portainer deployment instructions.
