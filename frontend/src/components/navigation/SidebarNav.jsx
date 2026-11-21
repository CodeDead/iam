import { NavLink, Stack } from '@mantine/core';
import {
  IconDashboard,
  IconKey,
  IconShieldLock,
  IconUserCircle,
  IconUsers,
} from '@tabler/icons-react';
import { useLocation, useNavigate } from 'react-router-dom';
import { usePermissions } from '../../hooks/usePermissions';

const NAV_ITEMS = [
  { label: 'Dashboard', path: '/', icon: IconDashboard, permission: null },
  { label: 'Users', path: '/users', icon: IconUsers, permission: 'users.read' },
  { label: 'Roles', path: '/roles', icon: IconShieldLock, permission: 'roles.read' },
  { label: 'Permissions', path: '/permissions', icon: IconKey, permission: 'permissions.read' },
  { label: 'My Profile', path: '/profile', icon: IconUserCircle, permission: 'profile.read' },
];

export function SidebarNav({ onNavigate }) {
  const location = useLocation();
  const navigate = useNavigate();
  const { hasPermission } = usePermissions();

  const visibleItems = NAV_ITEMS.filter(
    (item) => !item.permission || hasPermission(item.permission)
  );

  return (
    <Stack gap={4}>
      {visibleItems.map((item) => {
        const Icon = item.icon;
        const active =
          item.path === '/' ? location.pathname === '/' : location.pathname.startsWith(item.path);

        return (
          <NavLink
            key={item.path}
            label={item.label}
            leftSection={<Icon size={18} stroke={1.7} />}
            active={active}
            onClick={() => {
              navigate(item.path);
              if (onNavigate) onNavigate();
            }}
          />
        );
      })}
    </Stack>
  );
}
