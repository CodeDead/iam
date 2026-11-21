# CodeDead IAM — Frontend (React / JavaScript / Mantine UI)

Administration web application for **CodeDead IAM**, built with **React 19**, **JavaScript (ES6+)**, **React Router 7**, **Mantine UI 9**, **Vite 8**, and **Axios** (no TypeScript and no TanStack dependencies).

## Structure

- `src/services/` — Axios API client (`api.js` with in-memory access token, `withCredentials`, `X-CSRF-Token`, and concurrent `401` refresh coalescing) plus domain API modules.
- `src/contexts/` & `src/hooks/` — `AuthContext` / `useAuth` and `PermissionContext` / `usePermissions` (`hasPermission`, `hasAnyPermission`, `hasAllPermissions`).
- `src/routes/` — `AppRoutes` and `ProtectedRoute` (session restoration, permission gating, and post-logout redirect reset).
- `src/components/` — Reusable UI primitives (`PermissionGuard`, `PageHeader`, `StatusBadge`, `ErrorAlert`), `SidebarNav`, and grouped `PermissionMatrix`.
- `src/features/` — Feature pages and modals for `auth`, `dashboard`, `profile`, `users`, `roles`, and `permissions`.
- `src/test/` — Vitest 5 + React Testing Library test suites.

## Quick Start

```bash
cp .env.example .env
npm install
npm run dev
```

Open `http://localhost:5173`. Ensure the backend API is running on `VITE_API_BASE_URL` (default `http://localhost:8080/api/v1`).

## Commands

```bash
npm run format:check
npm run lint -- --max-warnings=0
npm test
npm run build
```

See the root [README.md](../README.md) for full-stack setup and Docker Compose instructions.
