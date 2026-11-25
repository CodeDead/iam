import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import AppRoutes from '../routes/AppRoutes';
import { profileService } from '../services/profileService';
import { renderWithProviders } from './testUtils';

vi.mock('../services/profileService', () => ({
  profileService: {
    getProfile: vi.fn(),
    updateProfile: vi.fn(),
    changePassword: vi.fn(),
    deleteAccount: vi.fn(),
  },
}));

const regularUser = {
  id: 'u-self',
  first_name: 'Jane',
  last_name: 'Member',
  email: 'jane@example.com',
  status: 'active',
  roles: [{ id: 'r4', name: 'User' }],
  profile: {},
};

describe('ProfilePage account deletion', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    profileService.getProfile.mockResolvedValue(regularUser);
  });

  it('allows a user with profile.delete to confirm password and permanently delete their account', async () => {
    const user = userEvent.setup();
    profileService.deleteAccount.mockResolvedValue({ success: true });

    renderWithProviders(<AppRoutes />, {
      initialEntries: ['/profile'],
      initialSession: {
        user: regularUser,
        permissions: ['profile.read', 'profile.update', 'profile.delete'],
      },
    });

    await user.click(await screen.findByRole('button', { name: /delete my account/i }));
    expect(await screen.findByText(/confirm account deletion/i)).toBeInTheDocument();

    await user.type(
      screen.getByLabelText(/^enter your current password to confirm/i),
      'StrongPass123!'
    );
    await user.click(screen.getByRole('button', { name: /permanently delete account/i }));

    await waitFor(() => {
      expect(profileService.deleteAccount).toHaveBeenCalledWith('StrongPass123!');
    });
    expect(await screen.findByRole('heading', { name: /sign in/i })).toBeInTheDocument();
  });

  it('shows API error message inside the confirmation modal when deletion fails', async () => {
    const user = userEvent.setup();
    profileService.deleteAccount.mockRejectedValue({
      response: {
        status: 400,
        data: {
          success: false,
          error: { code: 'INVALID_CURRENT_PASSWORD', message: 'Current password is incorrect' },
        },
      },
    });

    renderWithProviders(<AppRoutes />, {
      initialEntries: ['/profile'],
      initialSession: {
        user: regularUser,
        permissions: ['profile.read', 'profile.update', 'profile.delete'],
      },
    });

    await user.click(await screen.findByRole('button', { name: /delete my account/i }));
    await user.type(
      await screen.findByLabelText(/^enter your current password to confirm/i),
      'WrongPass123!'
    );
    await user.click(screen.getByRole('button', { name: /permanently delete account/i }));

    expect(
      (await screen.findAllByText(/current password is incorrect/i)).length
    ).toBeGreaterThanOrEqual(1);
  });

  it('displays LAST_SUPER_ADMIN_PROTECTED error message when deletion is rejected', async () => {
    const user = userEvent.setup();
    profileService.deleteAccount.mockRejectedValue({
      response: {
        status: 409,
        data: {
          code: 'LAST_SUPER_ADMIN_PROTECTED',
          message: 'Cannot delete the last active Super Administrator account',
        },
      },
    });

    renderWithProviders(<AppRoutes />, {
      initialEntries: ['/profile'],
      initialSession: {
        user: regularUser,
        permissions: ['profile.read', 'profile.update', 'profile.delete'],
      },
    });

    await user.click(await screen.findByRole('button', { name: /delete my account/i }));
    await user.type(
      await screen.findByLabelText(/^enter your current password to confirm/i),
      'SuperAdmin123!'
    );
    await user.click(screen.getByRole('button', { name: /permanently delete account/i }));

    expect(
      (await screen.findAllByText(/cannot delete the last active super administrator account/i))
        .length
    ).toBeGreaterThanOrEqual(1);
  });

  it('hides the delete account section when the user lacks profile.delete', async () => {
    renderWithProviders(<AppRoutes />, {
      initialEntries: ['/profile'],
      initialSession: {
        user: regularUser,
        permissions: ['profile.read', 'profile.update'],
      },
    });

    expect(await screen.findByText(/personal information/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /delete my account/i })).not.toBeInTheDocument();
  });
});
