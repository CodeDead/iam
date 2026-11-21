import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AppRoutes } from '../routes/AppRoutes';
import { authService } from '../services/authService';
import { dashboardService } from '../services/dashboardService';
import { roleService } from '../services/roleService';
import { userService } from '../services/userService';
import { renderWithProviders } from './testUtils';

vi.mock('../services/authService', () => ({
  authService: {
    login: vi.fn(),
    register: vi.fn(),
    logout: vi.fn(),
    refresh: vi.fn(),
    me: vi.fn(),
  },
}));

vi.mock('../services/dashboardService', () => ({
  dashboardService: {
    getStats: vi.fn(),
  },
}));

vi.mock('../services/userService', () => ({
  userService: {
    list: vi.fn(),
  },
}));

vi.mock('../services/roleService', () => ({
  roleService: {
    list: vi.fn(),
  },
}));

describe('Authentication, Registration, Logout & Route Protection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    dashboardService.getStats.mockResolvedValue({
      users: { total: 5, active: 4, disabled: 1, locked: 0, pending_verification: 0 },
      total_roles: 4,
      total_permissions: 16,
    });
  });

  it('redirects unauthenticated users from protected route to login', async () => {
    renderWithProviders(<AppRoutes />, { initialEntries: ['/users'], initialSession: null });
    expect(await screen.findByRole('heading', { name: /sign in/i })).toBeInTheDocument();
  });

  it('logs in user and navigates to dashboard', async () => {
    const user = userEvent.setup();
    authService.login.mockResolvedValue({
      access_token: 'access-123',
      csrf_token: 'csrf-123',
      user: {
        id: '1',
        first_name: 'Alice',
        last_name: 'Admin',
        email: 'alice@example.com',
        status: 'active',
        roles: [{ id: 'r1', name: 'Administrator' }],
      },
      permissions: ['users.read', 'roles.read', 'profile.read'],
    });

    renderWithProviders(<AppRoutes />, { initialEntries: ['/login'], initialSession: null });

    await user.type(screen.getByLabelText(/email address/i), 'alice@example.com');
    await user.type(screen.getByLabelText(/^password/i), 'StrongPass123!');
    await user.click(screen.getByRole('button', { name: /sign in/i }));

    await waitFor(() => {
      expect(authService.login).toHaveBeenCalledWith({
        email: 'alice@example.com',
        password: 'StrongPass123!',
      });
    });

    expect(await screen.findByText(/welcome, alice admin/i)).toBeInTheDocument();
  });

  it('displays API error message on failed login', async () => {
    const user = userEvent.setup();
    authService.login.mockRejectedValue({
      response: {
        status: 401,
        data: {
          success: false,
          error: { code: 'INVALID_CREDENTIALS', message: 'Invalid email or password' },
        },
      },
    });

    renderWithProviders(<AppRoutes />, { initialEntries: ['/login'], initialSession: null });

    await user.type(screen.getByLabelText(/email address/i), 'bad@example.com');
    await user.type(screen.getByLabelText(/^password/i), 'Wrong123!');
    await user.click(screen.getByRole('button', { name: /sign in/i }));

    expect(await screen.findByText(/invalid email or password/i)).toBeInTheDocument();
  });

  it('registers a new account with valid fields', async () => {
    const user = userEvent.setup();
    authService.register.mockResolvedValue({
      access_token: 'access-new',
      csrf_token: 'csrf-new',
      user: {
        id: '2',
        first_name: 'Jane',
        last_name: 'Doe',
        email: 'jane@example.com',
        status: 'active',
        roles: [{ id: 'r4', name: 'User' }],
      },
      permissions: ['profile.read', 'profile.update'],
    });

    renderWithProviders(<AppRoutes />, { initialEntries: ['/register'], initialSession: null });

    await user.type(screen.getByLabelText(/first name/i), 'Jane');
    await user.type(screen.getByLabelText(/last name/i), 'Doe');
    await user.type(screen.getByLabelText(/email address/i), 'jane@example.com');
    await user.type(screen.getByLabelText(/^password/i), 'StrongPass123!');
    await user.type(screen.getByLabelText(/confirm password/i), 'StrongPass123!');
    await user.click(screen.getByRole('button', { name: /register/i }));

    await waitFor(() => {
      expect(authService.register).toHaveBeenCalledWith({
        first_name: 'Jane',
        last_name: 'Doe',
        email: 'jane@example.com',
        password: 'StrongPass123!',
        password_confirmation: 'StrongPass123!',
      });
    });

    expect(await screen.findByText(/welcome, jane doe/i)).toBeInTheDocument();
  });

  it('redirects authenticated user lacking permission to forbidden page', async () => {
    const regularSession = {
      user: {
        id: '3',
        first_name: 'Basic',
        last_name: 'User',
        email: 'basic@example.com',
        status: 'active',
        roles: [{ id: 'r4', name: 'User' }],
      },
      permissions: ['profile.read', 'profile.update'],
    };

    renderWithProviders(<AppRoutes />, {
      initialEntries: ['/users'],
      initialSession: regularSession,
    });

    expect(await screen.findByText(/403 — access forbidden/i)).toBeInTheDocument();
  });

  it('logs out user and clears session', async () => {
    const user = userEvent.setup();
    authService.logout.mockResolvedValue();

    const session = {
      user: {
        id: '1',
        first_name: 'Alice',
        last_name: 'Admin',
        email: 'alice@example.com',
        status: 'active',
        roles: [{ id: 'r1', name: 'User' }],
      },
      permissions: ['profile.read'],
    };

    renderWithProviders(<AppRoutes />, { initialEntries: ['/'], initialSession: session });

    await user.click(await screen.findByText('Alice Admin'));
    await user.click(await screen.findByText(/sign out/i));

    await waitFor(() => {
      expect(authService.logout).toHaveBeenCalled();
    });
    expect(await screen.findByRole('heading', { name: /sign in/i })).toBeInTheDocument();
  });

  it('does not remember a restricted page after logout when a regular user logs in next', async () => {
    const user = userEvent.setup();
    authService.logout.mockResolvedValue();
    userService.list.mockResolvedValue({ items: [], total: 0, page: 1, total_pages: 1 });
    roleService.list.mockResolvedValue({ items: [], total: 0, page: 1, total_pages: 1 });

    const adminSession = {
      user: {
        id: '1',
        first_name: 'Super',
        last_name: 'Admin',
        email: 'admin@example.com',
        status: 'active',
        roles: [{ id: 'r1', name: 'Super Administrator' }],
      },
      permissions: ['users.read', 'roles.read', 'profile.read'],
    };

    authService.login.mockResolvedValue({
      access_token: 'regular-token',
      csrf_token: 'csrf-regular',
      user: {
        id: '2',
        first_name: 'Regular',
        last_name: 'User',
        email: 'regular@example.com',
        status: 'active',
        roles: [{ id: 'r4', name: 'User' }],
      },
      permissions: ['profile.read', 'profile.update'],
    });

    // Admin starts on restricted /users page and logs out
    renderWithProviders(<AppRoutes />, {
      initialEntries: ['/users'],
      initialSession: adminSession,
    });
    expect(await screen.findByText(/user management/i)).toBeInTheDocument();

    await user.click(screen.getByText('Super Admin'));
    await user.click(await screen.findByText(/sign out/i));
    expect(await screen.findByRole('heading', { name: /sign in/i })).toBeInTheDocument();

    // Regular user logs in and should land on Dashboard, NOT /users or 403 Forbidden
    await user.type(screen.getByLabelText(/email address/i), 'regular@example.com');
    await user.type(screen.getByLabelText(/^password/i), 'UserPass123!');
    await user.click(screen.getByRole('button', { name: /sign in/i }));

    expect(await screen.findByText(/welcome, regular user/i)).toBeInTheDocument();
    expect(screen.queryByText(/403 — access forbidden/i)).not.toBeInTheDocument();
  });

  it('skips startup session refresh when no csrf_token cookie is present', async () => {
    document.cookie = 'csrf_token=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/';

    renderWithProviders(<AppRoutes />, {
      initialEntries: ['/login'],
      initialSession: null,
      skipRestore: false,
    });

    expect(await screen.findByRole('heading', { name: /sign in/i })).toBeInTheDocument();
    expect(authService.refresh).not.toHaveBeenCalled();
  });
});
