import { useContext } from 'react';
import { PermissionContext } from '../contexts/contextInstances';

export function usePermissions() {
  const ctx = useContext(PermissionContext);
  if (!ctx) {
    throw new Error('usePermissions must be used within a PermissionProvider');
  }
  return ctx;
}
