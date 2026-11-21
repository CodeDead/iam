import { screen } from '@testing-library/react';
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
});
