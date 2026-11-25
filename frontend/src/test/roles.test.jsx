import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import PermissionMatrix from '../components/forms/PermissionMatrix';
import RolesPage from '../features/roles/RolesPage';
import { permissionService } from '../services/permissionService';
import { roleService } from '../services/roleService';
import { renderWithProviders } from './testUtils';

vi.mock('../services/roleService', () => ({
  roleService: {
    list: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    updatePermissions: vi.fn(),
    remove: vi.fn(),
    getById: vi.fn(),
    getAssignedUsers: vi.fn(),
  },
}));

vi.mock('../services/permissionService', () => ({
  permissionService: {
    list: vi.fn(),
  },
}));

describe('Role Management & Grouped Permission Matrix', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    permissionService.list.mockResolvedValue({
      items: [
        {
          id: 'p1',
          name: 'users.read',
          resource: 'users',
          action: 'read',
          description: 'Read users',
        },
        {
          id: 'p2',
          name: 'users.create',
          resource: 'users',
          action: 'create',
          description: 'Create users',
        },
        {
          id: 'p3',
          name: 'roles.read',
          resource: 'roles',
          action: 'read',
          description: 'Read roles',
        },
      ],
      total: 3,
    });
    roleService.list.mockResolvedValue({
      items: [
        {
          id: 'r1',
          name: 'Super Administrator',
          description: 'Full access',
          permission_ids: ['p1', 'p2', 'p3'],
          is_system: true,
        },
        {
          id: 'r2',
          name: 'Auditor',
          description: 'Read only auditor',
          permission_ids: ['p1'],
          is_system: false,
        },
      ],
      total: 2,
      page: 1,
      total_pages: 1,
    });
  });

  it('lists roles and hides delete button on protected system roles', async () => {
    const session = {
      user: { id: 'sa', email: 'sa@example.com', roles: [] },
      permissions: [
        'roles.read',
        'roles.create',
        'roles.update',
        'roles.delete',
        'permissions.read',
      ],
    };

    renderWithProviders(<RolesPage />, { initialSession: session });

    expect(await screen.findByText('Super Administrator')).toBeInTheDocument();
    expect(screen.getByText('Auditor')).toBeInTheDocument();
    expect(screen.queryByLabelText('Delete Super Administrator')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Delete Auditor')).toBeInTheDocument();
  });

  it('groups permissions by resource and supports toggling individual and group checkboxes', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const perms = [
      { id: 'p1', name: 'users.read', resource: 'users', action: 'read', description: 'Read' },
      {
        id: 'p2',
        name: 'users.create',
        resource: 'users',
        action: 'create',
        description: 'Create',
      },
    ];

    renderWithProviders(
      <PermissionMatrix allPermissions={perms} selectedIds={['p1']} onChange={onChange} />
    );

    expect(screen.getByText('Users')).toBeInTheDocument();
    await user.click(screen.getByLabelText('Create'));
    expect(onChange).toHaveBeenCalledWith(['p1', 'p2']);
  });
});
