import { screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DashboardPage } from '../features/dashboard/DashboardPage';
import { dashboardService } from '../services/dashboardService';
import { renderWithProviders } from './testUtils';

vi.mock('../services/dashboardService', () => ({
  dashboardService: {
    getStats: vi.fn(),
  },
}));

const fullAdminSession = {
  user: {
    id: 'u1',
    first_name: 'Super',
    last_name: 'Admin',
    email: 'admin@example.com',
    status: 'active',
    last_login_at: '2026-11-22T14:00:00Z',
    roles: [{ id: 'r1', name: 'Super Administrator' }],
  },
  permissions: ['users.read', 'roles.read', 'permissions.read', 'audit_logs.read', 'profile.read'],
};

const standardUserSession = {
  user: {
    id: 'u2',
    first_name: 'Regular',
    last_name: 'User',
    email: 'user@example.com',
    status: 'active',
    last_login_at: '2026-11-22T14:00:00Z',
    roles: [{ id: 'r4', name: 'User' }],
  },
  permissions: ['profile.read', 'profile.update', 'profile.delete'],
};

const fullStatsPayload = {
  my_active_sessions: 2,
  users: {
    total: 12,
    active: 10,
    disabled: 1,
    locked: 1,
    pending_verification: 0,
    new_last_7_days: 3,
    active_sessions: 5,
  },
  total_roles: 5,
  roles: {
    total: 5,
    system: 4,
    custom: 1,
    distribution: [
      {
        id: 'r1',
        name: 'Super Administrator',
        is_system: true,
        permission_count: 18,
        user_count: 1,
      },
      {
        id: 'r5',
        name: 'Compliance Auditor',
        is_system: false,
        permission_count: 3,
        user_count: 2,
      },
    ],
  },
  total_permissions: 18,
  permissions: {
    total: 18,
    system: 18,
    custom: 0,
    by_resource: [
      { resource: 'users', count: 6 },
      { resource: 'roles', count: 4 },
    ],
  },
  audit: {
    total: 42,
    events_last_24h: 8,
    events_last_7d: 25,
    failed_logins_last_24h: 2,
    token_reuse_alerts_last_7d: 1,
    daily_activity: [
      { date: '2026-11-16', total: 2, auth_success: 2, auth_failed: 0, admin_changes: 0 },
      { date: '2026-11-17', total: 4, auth_success: 2, auth_failed: 1, admin_changes: 1 },
      { date: '2026-11-18', total: 0, auth_success: 0, auth_failed: 0, admin_changes: 0 },
      { date: '2026-11-19', total: 5, auth_success: 3, auth_failed: 0, admin_changes: 2 },
      { date: '2026-11-20', total: 3, auth_success: 2, auth_failed: 0, admin_changes: 1 },
      { date: '2026-11-21', total: 3, auth_success: 2, auth_failed: 0, admin_changes: 1 },
      { date: '2026-11-22', total: 8, auth_success: 4, auth_failed: 2, admin_changes: 2 },
    ],
    by_resource_type: [
      { resource: 'auth', count: 18 },
      { resource: 'user', count: 7 },
    ],
    recent_events: [
      {
        id: 'ev1',
        action: 'auth.login_failed',
        resource_type: 'auth',
        actor_email: 'attacker@example.com',
        summary: "Failed login attempt for 'admin@example.com'",
        created_at: '2026-11-22T15:00:00Z',
      },
    ],
  },
};

describe('DashboardPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders KPI cards, security alert banner, charts, and recent audit activity for administrators', async () => {
    dashboardService.getStats.mockResolvedValue(fullStatsPayload);

    renderWithProviders(<DashboardPage />, { initialSession: fullAdminSession });

    expect(await screen.findByText(/refresh token reuse detected/i)).toBeInTheDocument();
    expect(screen.getByText(/7-day security & audit activity/i)).toBeInTheDocument();
    expect(screen.getByText(/user account status/i)).toBeInTheDocument();
    expect(screen.getByText(/role coverage & distribution/i)).toBeInTheDocument();
    expect(screen.getByText(/permissions by resource/i)).toBeInTheDocument();
    expect(screen.getByText(/audit events by resource \(7d\)/i)).toBeInTheDocument();
    expect(screen.getByText(/recent security & audit activity/i)).toBeInTheDocument();
    expect(screen.getByText(/failed login attempt for 'admin@example.com'/i)).toBeInTheDocument();
    expect(screen.getByText('Compliance Auditor')).toBeInTheDocument();
    expect(
      screen.getByRole('img', { name: /7-day audit activity bar chart showing 25 total events/i })
    ).toBeInTheDocument();
  });

  it('hides privileged cards, charts, and audit feed for standard users without admin permissions', async () => {
    dashboardService.getStats.mockResolvedValue({
      my_active_sessions: 1,
    });

    renderWithProviders(<DashboardPage />, { initialSession: standardUserSession });

    expect(await screen.findByText(/your active sessions/i)).toBeInTheDocument();
    expect(screen.getByText(/your access & quick navigation/i)).toBeInTheDocument();
    expect(screen.queryByText(/7-day security & audit activity/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/user account status/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/role coverage & distribution/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/permissions by resource/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/recent security & audit activity/i)).not.toBeInTheDocument();
  });

  it('displays an error alert when dashboard stats fail to load', async () => {
    dashboardService.getStats.mockRejectedValue({
      code: 'ERR_NETWORK',
      message: 'Network Error',
      request: {},
    });

    renderWithProviders(<DashboardPage />, { initialSession: fullAdminSession });

    expect(await screen.findByText(/unable to connect to the server/i)).toBeInTheDocument();
  });
});
