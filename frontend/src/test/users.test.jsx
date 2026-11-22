import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { UsersPage } from '../features/users/UsersPage';
import { roleService } from '../services/roleService';
import { userService } from '../services/userService';
import { renderWithProviders } from './testUtils';

vi.mock('../services/userService', () => ({
  userService: {
    list: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    updateStatus: vi.fn(),
    remove: vi.fn(),
    updateRoles: vi.fn(),
    getById: vi.fn(),
    getEffectivePermissions: vi.fn(),
    getOrganisations: vi.fn(),
  },
}));

vi.mock('../services/roleService', () => ({
  roleService: {
    list: vi.fn(),
  },
}));

describe('User Management Page', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    roleService.list.mockResolvedValue({ items: [], total: 0 });
    userService.list.mockResolvedValue({
      items: [
        {
          id: 'u1',
          first_name: 'Alice',
          last_name: 'Smith',
          email: 'alice@example.com',
          status: 'active',
          roles: [{ id: 'r1', name: 'Administrator' }],
          last_login_at: '2026-11-20T10:00:00Z',
        },
        {
          id: 'u2',
          first_name: 'Bob',
          last_name: 'Jones',
          email: 'bob@example.com',
          status: 'disabled',
          roles: [{ id: 'r2', name: 'User' }],
          last_login_at: null,
        },
      ],
      total: 2,
      page: 1,
      total_pages: 1,
    });
  });

  it('lists users and shows Create User button when authorized', async () => {
    const adminSession = {
      user: { id: 'u1', email: 'alice@example.com', roles: [] },
      permissions: ['users.read', 'users.create', 'users.update', 'roles.read'],
    };

    renderWithProviders(<UsersPage />, { initialSession: adminSession });

    expect(await screen.findByText('alice@example.com')).toBeInTheDocument();
    expect(screen.getByText('bob@example.com')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /create user/i })).toBeInTheDocument();
  });

  it('hides Create User button when user only has users.read', async () => {
    const readerSession = {
      user: { id: 'u3', email: 'reader@example.com', roles: [] },
      permissions: ['users.read'],
    };

    renderWithProviders(<UsersPage />, { initialSession: readerSession });

    expect(await screen.findByText('alice@example.com')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /create user/i })).not.toBeInTheDocument();
  });

  it('displays an error alert when the backend is offline during user lookup', async () => {
    userService.list.mockRejectedValue({
      code: 'ERR_NETWORK',
      message: 'Network Error',
    });

    const adminSession = {
      user: { id: 'u1', email: 'alice@example.com', roles: [] },
      permissions: ['users.read'],
    };

    renderWithProviders(<UsersPage />, { initialSession: adminSession });

    expect(
      await screen.findByText(/unable to connect to the server\. please verify the api is running/i)
    ).toBeInTheDocument();
  });

  it('shows user organisation memberships in the detail drawer when organisations.read is granted', async () => {
    const user = userEvent.setup();
    userService.getById.mockResolvedValue({
      id: 'u1',
      first_name: 'Alice',
      last_name: 'Smith',
      email: 'alice@example.com',
      status: 'active',
      email_verified: true,
      roles: [{ id: 'r1', name: 'Administrator' }],
      created_at: '2026-11-20T10:00:00Z',
      updated_at: '2026-11-20T10:00:00Z',
      last_login_at: '2026-11-20T10:00:00Z',
    });
    userService.getEffectivePermissions.mockResolvedValue([
      { id: 'p1', name: 'users.read' },
      { id: 'p2', name: 'organisations.read' },
    ]);
    userService.getOrganisations.mockResolvedValue([
      {
        id: 'org1',
        name: 'Acme Inc.',
        role: 'owner',
        status: 'active',
        member_count: 3,
        active_member_count: 2,
        active_owner_count: 1,
      },
    ]);

    const adminSession = {
      user: { id: 'u1', email: 'alice@example.com', roles: [] },
      permissions: ['users.read', 'organisations.read'],
    };

    renderWithProviders(<UsersPage />, { initialSession: adminSession });

    expect(await screen.findByText('alice@example.com')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /view alice@example\.com/i }));

    expect(await screen.findByText('Organisations (1)')).toBeInTheDocument();
    const orgLink = screen.getByRole('link', { name: 'Acme Inc.' });
    expect(orgLink).toHaveAttribute('href', '/organisations?search=Acme%20Inc.');
    expect(userService.getOrganisations).toHaveBeenCalledWith('u1');
  });
});
