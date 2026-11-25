import { lazy, Suspense } from 'react';
import { Navigate, Route, Routes, useLocation } from 'react-router-dom';
import RouteErrorBoundary from '../components/common/RouteErrorBoundary';
import RouteFallback from '../components/common/RouteFallback';
import ProtectedRoute from './ProtectedRoute';

const AuthLayout = lazy(() => import('../layouts/AuthLayout'));
const DashboardLayout = lazy(() => import('../layouts/DashboardLayout'));

const AuditLogsPage = lazy(() => import('../features/audit/AuditLogsPage'));
const ForbiddenPage = lazy(() => import('../features/auth/ForbiddenPage'));
const LoginPage = lazy(() => import('../features/auth/LoginPage'));
const RegisterPage = lazy(() => import('../features/auth/RegisterPage'));
const UnauthorizedPage = lazy(() => import('../features/auth/UnauthorizedPage'));
const DashboardPage = lazy(() => import('../features/dashboard/DashboardPage'));
const OrganisationsPage = lazy(() => import('../features/organisations/OrganisationsPage'));
const PermissionsPage = lazy(() => import('../features/permissions/PermissionsPage'));
const ProfilePage = lazy(() => import('../features/profile/ProfilePage'));
const RolesPage = lazy(() => import('../features/roles/RolesPage'));
const UsersPage = lazy(() => import('../features/users/UsersPage'));

const AppRoutes = () => {
  const location = useLocation();

  return (
    <RouteErrorBoundary resetKey={location.pathname}>
      <Suspense fallback={<RouteFallback />}>
        <Routes>
          <Route element={<AuthLayout />}>
            <Route path="/login" element={<LoginPage />} />
            <Route path="/register" element={<RegisterPage />} />
          </Route>

          <Route path="/unauthorized" element={<UnauthorizedPage />} />
          <Route path="/forbidden" element={<ForbiddenPage />} />

          <Route
            element={
              <ProtectedRoute>
                <DashboardLayout />
              </ProtectedRoute>
            }
          >
            <Route index element={<DashboardPage />} />
            <Route
              path="/users"
              element={
                <ProtectedRoute requiredPermission="users.read">
                  <UsersPage />
                </ProtectedRoute>
              }
            />
            <Route path="/organisations" element={<OrganisationsPage />} />
            <Route
              path="/roles"
              element={
                <ProtectedRoute requiredPermission="roles.read">
                  <RolesPage />
                </ProtectedRoute>
              }
            />
            <Route
              path="/permissions"
              element={
                <ProtectedRoute requiredPermission="permissions.read">
                  <PermissionsPage />
                </ProtectedRoute>
              }
            />
            <Route
              path="/audit-logs"
              element={
                <ProtectedRoute requiredPermission="audit_logs.read">
                  <AuditLogsPage />
                </ProtectedRoute>
              }
            />
            <Route
              path="/profile"
              element={
                <ProtectedRoute requiredPermission="profile.read">
                  <ProfilePage />
                </ProtectedRoute>
              }
            />
          </Route>

          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </Suspense>
    </RouteErrorBoundary>
  );
};

export default AppRoutes;
