import { screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PermissionGuard } from '../components/common/PermissionGuard';
import { SidebarNav } from '../components/navigation/SidebarNav';
import { PermissionsPage } from '../features/permissions/PermissionsPage';
import { ProfilePage } from '../features/profile/ProfilePage';
import { permissionService } from '../services/permissionService';
import { profileService } from '../services/profileService';
import { renderWithProviders } from './testUtils';

vi.mock('../services/permissionService', () => ({
  permissionService: {
    list: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    remove: vi.fn(),
    getAssociatedRoles: vi.fn(),
  },
}));

vi.mock('../services/profileService', () => ({
  profileService: {
    getProfile: vi.fn(),
    updateProfile: vi.fn(),
    changePassword: vi.fn(),
  },
}));

describe('PermissionGuard, Navigation Visibility & Permission Management', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders protected UI only when user holds required permissions', () => {
    const session = {
      user: { id: '1', email: 'u@example.com', roles: [] },
      permissions: ['users.read', 'roles.read'],
    };

    renderWithProviders(
      <div>
        <PermissionGuard permission="users.read">
          <span>Can Read Users</span>
        </PermissionGuard>
        <PermissionGuard permission="users.create">
          <span>Can Create Users</span>
        </PermissionGuard>
        <PermissionGuard anyPermissions={['users.delete', 'roles.read']}>
          <span>Any Match</span>
        </PermissionGuard>
        <PermissionGuard allPermissions={['users.read', 'roles.update']}>
          <span>All Match Missing</span>
        </PermissionGuard>
      </div>,
      { initialSession: session }
    );

    expect(screen.getByText('Can Read Users')).toBeInTheDocument();
    expect(screen.queryByText('Can Create Users')).not.toBeInTheDocument();
    expect(screen.getByText('Any Match')).toBeInTheDocument();
    expect(screen.queryByText('All Match Missing')).not.toBeInTheDocument();
  });

  it('hides administrative sidebar links for regular users', () => {
    const regularSession = {
      user: { id: '2', email: 'user@example.com', roles: [] },
      permissions: ['profile.read', 'profile.update'],
    };

    renderWithProviders(<SidebarNav />, { initialSession: regularSession });

    expect(screen.getByText('Dashboard')).toBeInTheDocument();
    expect(screen.getByText('My Profile')).toBeInTheDocument();
    expect(screen.queryByText('Users')).not.toBeInTheDocument();
    expect(screen.queryByText('Roles')).not.toBeInTheDocument();
    expect(screen.queryByText('Permissions')).not.toBeInTheDocument();
  });

  it('distinguishes system vs custom permissions and protects system ones from edit/delete', async () => {
    permissionService.list.mockResolvedValue({
      items: [
        {
          id: 'p1',
          name: 'users.read',
          resource: 'users',
          action: 'read',
          description: 'Read users',
          is_system: true,
        },
        {
          id: 'p2',
          name: 'reports.export',
          resource: 'reports',
          action: 'export',
          description: 'Export reports',
          is_system: false,
        },
      ],
      total: 2,
      page: 1,
      total_pages: 1,
    });

    const superAdminSession = {
      user: { id: 'sa', email: 'sa@example.com', roles: [] },
      permissions: [
        'permissions.read',
        'permissions.create',
        'permissions.update',
        'permissions.delete',
        'roles.read',
      ],
    };

    renderWithProviders(<PermissionsPage />, { initialSession: superAdminSession });

    expect(await screen.findByText('users.read')).toBeInTheDocument();
    expect(screen.getByText('reports.export')).toBeInTheDocument();
    expect(screen.getByText('System')).toBeInTheDocument();
    expect(screen.getByText('Custom')).toBeInTheDocument();

    // System permission should NOT have edit/delete buttons; custom permission SHOULD
    expect(screen.queryByLabelText('Edit users.read')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Delete users.read')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Edit reports.export')).toBeInTheDocument();
    expect(screen.getByLabelText('Delete reports.export')).toBeInTheDocument();
  });

  it('hides Account Access Summary on ProfilePage for normal users and shows it for authorized users', async () => {
    const baseUser = {
      id: 'u1',
      first_name: 'Normal',
      last_name: 'User',
      email: 'normal@example.com',
      status: 'active',
      roles: [{ id: 'r1', name: 'User' }],
      profile: {},
    };
    profileService.getProfile.mockResolvedValue(baseUser);

    const { unmount } = renderWithProviders(<ProfilePage />, {
      initialSession: {
        user: baseUser,
        permissions: ['profile.read', 'profile.update'],
      },
    });

    expect(await screen.findByText('Personal Information')).toBeInTheDocument();
    expect(screen.queryByText('Account Access Summary')).not.toBeInTheDocument();
    unmount();

    const adminUser = {
      ...baseUser,
      roles: [{ id: 'r2', name: 'Administrator' }],
    };
    profileService.getProfile.mockResolvedValue(adminUser);

    renderWithProviders(<ProfilePage />, {
      initialSession: {
        user: adminUser,
        permissions: ['profile.read', 'profile.update', 'roles.read'],
      },
    });

    expect(await screen.findByText('Account Access Summary')).toBeInTheDocument();
  });
});
