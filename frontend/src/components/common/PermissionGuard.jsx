import { usePermissions } from '../../hooks/usePermissions';

const PermissionGuard = ({
  permission,
  anyPermissions,
  allPermissions,
  fallback = null,
  children,
}) => {
  const { hasPermission, hasAnyPermission, hasAllPermissions } = usePermissions();

  let allowed = true;
  if (permission) {
    allowed = allowed && hasPermission(permission);
  }
  if (anyPermissions && anyPermissions.length > 0) {
    allowed = allowed && hasAnyPermission(anyPermissions);
  }
  if (allPermissions && allPermissions.length > 0) {
    allowed = allowed && hasAllPermissions(allPermissions);
  }

  return allowed ? children : fallback;
};

export default PermissionGuard;
