import { Center, Loader, Stack, Text } from '@mantine/core';
import { Navigate, useLocation } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { usePermissions } from '../hooks/usePermissions';

const ProtectedRoute = ({ children, requiredPermission, anyPermissions, allPermissions }) => {
  const { isAuthenticated, isLoading, wasLoggedOut } = useAuth();
  const { hasPermission, hasAnyPermission, hasAllPermissions } = usePermissions();
  const location = useLocation();

  if (isLoading) {
    return (
      <Center component="main" mih="60vh">
        <Stack align="center" gap="sm">
          <Loader size="md" />
          <Text size="sm" c="dimmed">
            Restoring session...
          </Text>
        </Stack>
      </Center>
    );
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" state={wasLoggedOut ? null : { from: location }} replace />;
  }

  if (requiredPermission && !hasPermission(requiredPermission)) {
    return <Navigate to="/forbidden" replace />;
  }

  if (anyPermissions && anyPermissions.length > 0 && !hasAnyPermission(anyPermissions)) {
    return <Navigate to="/forbidden" replace />;
  }

  if (allPermissions && allPermissions.length > 0 && !hasAllPermissions(allPermissions)) {
    return <Navigate to="/forbidden" replace />;
  }

  return children;
};

export default ProtectedRoute;
