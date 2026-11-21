# CodeDead IAM — React Administration Frontend

JavaScript (ES6+) administration SPA built with **React 19**, **React Router 7**, **Mantine UI 9**, **Axios**, and **Vite 8**.

## Stack Rules

- Pure JavaScript (no TypeScript).
- No TanStack dependencies (`@tanstack/*`).
- Consistent Mantine UI components & hooks across all screens.
- Access tokens kept strictly in memory (never `localStorage`); HttpOnly refresh-token cookies + `X-CSRF-Token` double-submit header with concurrent 401 refresh coalescing (`src/services/api.js`).

## Structure

- `src/config/api.js` — API base URL and CSRF header/cookie constants.
- `src/services/` — Axios client with automatic refresh queue plus feature API services.
- `src/contexts/` & `src/hooks/` — `AuthContext`, `PermissionContext`, `useAuth`, `usePermissions`.
- `src/routes/` — `AppRoutes` and permission-aware `ProtectedRoute`.
- `src/layouts/` — `AuthLayout` and responsive `DashboardLayout`.
- `src/components/` — `PermissionGuard`, `PermissionMatrix` (grouped by resource), `SidebarNav`, `StatusBadge`, `ErrorAlert`, `PageHeader`.
- `src/features/` — `auth`, `dashboard`, `profile`, `users`, `roles`, `permissions`.
- `src/test/` — Vitest 5 + React Testing Library test suites.

## Local Commands

```bash
cp .env.example .env
npm install

# Start Vite dev server on http://localhost:5173
npm run dev

# Quality checks, unit/UI tests & production build
npm run format:check
npm run lint -- --max-warnings=0
npm test
npm run build
```
