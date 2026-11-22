import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AuditLogsPage } from '../features/audit/AuditLogsPage';
import { auditService } from '../services/auditService';
import { renderWithProviders } from './testUtils';

vi.mock('../services/auditService', () => ({
  auditService: {
    list: vi.fn(),
    getById: vi.fn(),
  },
}));

const adminSession = {
  user: {
    id: 'u1',
    first_name: 'Super',
    last_name: 'Admin',
    email: 'admin@example.com',
    status: 'active',
    roles: [{ id: 'r1', name: 'Super Administrator' }],
  },
  permissions: ['audit_logs.read', 'profile.read'],
};

describe('AuditLogsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    auditService.list.mockResolvedValue({
      items: [
        {
          id: 'log1',
          action: 'permission.created',
          resource_type: 'permission',
          resource_id: 'p1',
          resource_name: 'billing.read',
          actor_id: 'u1',
          actor_email: 'admin@example.com',
          summary: "Created permission 'billing.read'",
          details: { resource: 'billing', action: 'read', description: 'View invoices' },
          created_at: '2026-11-22T00:00:00Z',
        },
        {
          id: 'log2',
          action: 'role.created',
          resource_type: 'role',
          resource_id: 'r2',
          resource_name: 'Auditor',
          actor_id: 'u1',
          actor_email: 'admin@example.com',
          summary: "Created role 'Auditor'",
          details: { description: 'Compliance auditor', permissions: ['billing.read'] },
          created_at: '2026-11-22T00:01:00Z',
        },
      ],
      total: 2,
      page: 1,
      total_pages: 1,
    });
  });

  it('renders audit log entries and opens the event detail modal', async () => {
    const user = userEvent.setup();
    renderWithProviders(<AuditLogsPage />, { initialSession: adminSession });

    expect(await screen.findByText(/created permission 'billing.read'/i)).toBeInTheDocument();
    expect(screen.getByText(/created role 'auditor'/i)).toBeInTheDocument();

    await user.click(
      screen.getByRole('button', { name: /inspect audit entry permission\.created/i })
    );
    expect(await screen.findByText(/audit event details/i)).toBeInTheDocument();
    expect(screen.getByText(/"resource": "billing"/i)).toBeInTheDocument();
  });

  it('displays an error alert when loading audit logs fails offline', async () => {
    auditService.list.mockRejectedValue({
      code: 'ERR_NETWORK',
      message: 'Network Error',
      request: {},
    });

    renderWithProviders(<AuditLogsPage />, { initialSession: adminSession });

    expect(await screen.findByText(/unable to connect to the server/i)).toBeInTheDocument();
  });
});
