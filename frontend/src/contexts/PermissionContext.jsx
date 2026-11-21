import { useCallback, useMemo } from 'react';
import { useAuth } from '../hooks/useAuth';
import { PermissionContext } from './contextInstances';

export function PermissionProvider({ children }) {
  const { permissions } = useAuth();

  const permissionSet = useMemo(() => new Set(permissions || []), [permissions]);

  const hasPermission = useCallback(
    (permission) => {
      if (!permission) return true;
      return permissionSet.has(permission);
    },
    [permissionSet]
  );

  const hasAnyPermission = useCallback(
    (requiredList = []) => {
      if (!requiredList || requiredList.length === 0) return true;
      return requiredList.some((p) => permissionSet.has(p));
    },
    [permissionSet]
  );

  const hasAllPermissions = useCallback(
    (requiredList = []) => {
      if (!requiredList || requiredList.length === 0) return true;
      return requiredList.every((p) => permissionSet.has(p));
    },
    [permissionSet]
  );

  const value = useMemo(
    () => ({
      permissions,
      hasPermission,
      hasAnyPermission,
      hasAllPermissions,
    }),
    [permissions, hasPermission, hasAnyPermission, hasAllPermissions]
  );

  return <PermissionContext.Provider value={value}>{children}</PermissionContext.Provider>;
}
