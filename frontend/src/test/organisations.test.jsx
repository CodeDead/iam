import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Route, Routes } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import RegisterPage from '../features/auth/RegisterPage';
import OrganisationsPage from '../features/organisations/OrganisationsPage';
import { authService } from '../services/authService';
import { organisationService } from '../services/organisationService';
import { renderWithProviders } from './testUtils';

vi.mock('../services/organisationService', () => ({
  organisationService: {
    listMine: vi.fn(),
    list: vi.fn(),
    getById: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    remove: vi.fn(),
    addMember: vi.fn(),
    updateMember: vi.fn(),
    removeMember: vi.fn(),
    acceptInvitation: vi.fn(),
    leave: vi.fn(),
  },
}));

vi.mock('../services/authService', () => ({
  authService: {
    login: vi.fn(),
    register: vi.fn(),
    logout: vi.fn(),
    refresh: vi.fn(),
    me: vi.fn(),
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
  permissions: [
    'organisations.read',
    'organisations.create',
    'organisations.update',
    'organisations.delete',
    'organisations.manage_members',
  ],
};

const orgOwnerSession = {
  user: {
    id: 'u10',
    first_name: 'Alice',
    last_name: 'Owner',
    email: 'alice@acme.test',
    status: 'active',
    roles: [{ id: 'r4', name: 'User' }],
  },
  permissions: ['profile.read', 'profile.update', 'profile.delete'],
};

const sampleOrg = {
  id: 'org1',
  name: 'Acme Inc.',
  member_count: 1,
  active_member_count: 1,
  members: [
    {
      user_id: 'u1',
      email: 'admin@example.com',
      first_name: 'Super',
      last_name: 'Admin',
      role: 'owner',
      status: 'active',
      joined_at: '2026-11-22T12:00:00Z',
    },
  ],
  created_at: '2026-11-22T12:00:00Z',
  updated_at: '2026-11-22T12:00:00Z',
};

describe('OrganisationsPage & Organisation Registration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    organisationService.listMine.mockResolvedValue([]);
    organisationService.list.mockResolvedValue({
      items: [sampleOrg],
      total: 1,
      page: 1,
      total_pages: 1,
    });
  });

  it('lists organisations and allows admins to add a member by signup email', async () => {
    const user = userEvent.setup();
    organisationService.addMember.mockResolvedValue({
      ...sampleOrg,
      member_count: 2,
      active_member_count: 2,
      members: [
        ...sampleOrg.members,
        {
          user_id: 'u2',
          email: 'alice@acme.test',
          first_name: 'Alice',
          last_name: 'Member',
          role: 'member',
          status: 'active',
          joined_at: '2026-11-22T13:00:00Z',
        },
      ],
    });

    renderWithProviders(<OrganisationsPage />, { initialSession: adminSession });

    expect(await screen.findByText('Acme Inc.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /manage members of acme inc\./i }));

    const emailInput = await screen.findByLabelText(/add user by signup email/i);
    await user.type(emailInput, 'alice@acme.test');
    await user.click(screen.getByRole('button', { name: /add member/i }));

    expect(organisationService.addMember).toHaveBeenCalledWith('org1', {
      email: 'alice@acme.test',
      role: 'member',
      status: 'active',
    });
    expect(await screen.findByText('alice@acme.test')).toBeInTheDocument();
  });

  it('allows an organisation owner without global permissions to invite by email with privacy protection, remove members, and accept invitations', async () => {
    const user = userEvent.setup();
    organisationService.listMine.mockResolvedValue([
      {
        id: 'org10',
        name: 'Alpha Org',
        role: 'owner',
        status: 'active',
        member_count: 1,
        joined_at: '2026-11-22T12:00:00Z',
        created_at: '2026-11-22T12:00:00Z',
      },
      {
        id: 'org20',
        name: 'Beta Org',
        role: 'member',
        status: 'invited',
        member_count: 2,
        joined_at: '2026-11-22T12:00:00Z',
        created_at: '2026-11-22T12:00:00Z',
      },
    ]);

    const alphaDetails = {
      id: 'org10',
      name: 'Alpha Org',
      member_count: 1,
      active_member_count: 1,
      members: [
        {
          user_id: 'u10',
          email: 'alice@acme.test',
          first_name: 'Alice',
          last_name: 'Owner',
          role: 'owner',
          status: 'active',
          joined_at: '2026-11-22T12:00:00Z',
        },
      ],
    };

    const alphaWithBobInvited = {
      ...alphaDetails,
      member_count: 2,
      members: [
        ...alphaDetails.members,
        {
          user_id: 'u11',
          email: 'bob@shared.test',
          role: 'member',
          status: 'invited',
          joined_at: '2026-11-22T13:00:00Z',
        },
      ],
    };

    organisationService.getById.mockResolvedValue(alphaDetails);
    organisationService.addMember.mockResolvedValue(alphaWithBobInvited);
    organisationService.removeMember.mockResolvedValue(alphaDetails);
    organisationService.acceptInvitation.mockResolvedValue({
      id: 'org20',
      name: 'Beta Org',
      role: 'member',
      status: 'active',
      member_count: 2,
    });

    renderWithProviders(<OrganisationsPage />, { initialSession: orgOwnerSession });

    // Sees pending invitation to Beta Org and can accept it
    expect(await screen.findByText(/pending organisation invitation/i)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /accept/i }));
    expect(organisationService.acceptInvitation).toHaveBeenCalledWith('org20');

    // Never calls global list endpoint without organisations.read
    expect(organisationService.list).not.toHaveBeenCalled();

    // Opens members modal for Alpha Org and invites Bob by signup email
    await user.click(screen.getByRole('button', { name: /manage members of alpha org/i }));
    const emailInput = await screen.findByLabelText(/add user by signup email/i);
    await user.type(emailInput, 'bob@shared.test');
    await user.click(screen.getByRole('button', { name: /invite user/i }));

    expect(organisationService.addMember).toHaveBeenCalledWith('org10', {
      email: 'bob@shared.test',
      role: 'member',
    });
    expect(
      await screen.findByText(/name hidden until invitation is accepted/i)
    ).toBeInTheDocument();

    // Cancels Bob's pending invitation in Alpha Org
    await user.click(
      screen.getByRole('button', { name: /cancel invitation for bob@shared\.test/i })
    );
    expect(organisationService.removeMember).toHaveBeenCalledWith('org10', 'u11');

    // Close modal and verify org owner can delete their own organisation
    await user.keyboard('{Escape}');
    organisationService.remove.mockResolvedValue(undefined);
    await user.click(screen.getByRole('button', { name: /delete alpha org/i }));
    await user.click(await screen.findByRole('button', { name: /^delete$/i }));
    expect(organisationService.remove).toHaveBeenCalledWith('org10');

    // Regular user can also create a new organisation directly from OrganisationsPage
    organisationService.create.mockResolvedValue({
      id: 'org30',
      name: 'My Startup',
      member_count: 1,
      active_member_count: 1,
      members: [],
    });
    await user.click(screen.getByRole('button', { name: /create organisation/i }));
    const createDialog = await screen.findByRole('dialog');
    await user.type(within(createDialog).getByLabelText(/organisation name/i), 'My Startup');
    await user.click(within(createDialog).getByRole('button', { name: /create organisation/i }));
    expect(organisationService.create).toHaveBeenCalledWith({ name: 'My Startup' });
  });

  it('allows registering a new organisation account from RegisterPage', async () => {
    const user = userEvent.setup();
    authService.register.mockResolvedValue({
      access_token: 'tok',
      user: {
        id: 'u9',
        first_name: 'Jane',
        last_name: 'Founder',
        email: 'jane@acme.test',
        roles: [],
      },
      permissions: ['profile.read'],
      organisation: {
        id: 'org9',
        name: 'Acme Inc.',
        role: 'owner',
        status: 'active',
        member_count: 1,
      },
    });

    renderWithProviders(
      <Routes>
        <Route path="/register" element={<RegisterPage />} />
        <Route path="/" element={<div>Dashboard Home</div>} />
      </Routes>,
      { initialEntries: ['/register'] }
    );

    await user.click(screen.getByText(/organisation account/i));
    await user.type(screen.getByLabelText(/organisation name/i), 'Acme Inc.');
    await user.type(screen.getByLabelText(/first name/i), 'Jane');
    await user.type(screen.getByLabelText(/last name/i), 'Founder');
    await user.type(screen.getByLabelText(/email address/i), 'jane@acme.test');
    await user.type(screen.getByLabelText(/^password/i), 'StrongPass123!');
    await user.type(screen.getByLabelText(/confirm password/i), 'StrongPass123!');
    await user.click(screen.getByRole('button', { name: /register organisation/i }));

    expect(authService.register).toHaveBeenCalledWith({
      first_name: 'Jane',
      last_name: 'Founder',
      email: 'jane@acme.test',
      password: 'StrongPass123!',
      password_confirmation: 'StrongPass123!',
      organisation_name: 'Acme Inc.',
    });
    expect(await screen.findByText('Dashboard Home')).toBeInTheDocument();
  });

  it('preserves org-owner management controls in All Organisations for a Moderator who owns an org and protects the last active owner', async () => {
    const user = userEvent.setup();
    const moderatorOwnerSession = {
      user: {
        id: 'u10',
        first_name: 'Alice',
        last_name: 'Mod',
        email: 'alice@acme.test',
        status: 'active',
        roles: [{ id: 'r3', name: 'Moderator' }],
      },
      permissions: ['organisations.read', 'profile.read'],
    };

    const ownedOrg = {
      id: 'org99',
      name: 'Owned By Mod Co',
      member_count: 1,
      active_member_count: 1,
      members: [
        {
          user_id: 'u10',
          email: 'alice@acme.test',
          first_name: 'Alice',
          last_name: 'Mod',
          role: 'owner',
          status: 'active',
          joined_at: '2026-11-22T12:00:00Z',
        },
      ],
      created_at: '2026-11-22T12:00:00Z',
      updated_at: '2026-11-22T12:00:00Z',
    };

    organisationService.listMine.mockResolvedValue([]);
    organisationService.list.mockResolvedValue({
      items: [ownedOrg],
      total: 1,
      page: 1,
      total_pages: 1,
    });

    renderWithProviders(<OrganisationsPage />, { initialSession: moderatorOwnerSession });

    expect(await screen.findByText('Owned By Mod Co')).toBeInTheDocument();
    // Moderator who owns the org gets full manage/edit/delete controls in All Organisations
    expect(screen.getByRole('button', { name: /edit owned by mod co/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /delete owned by mod co/i })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /manage members of owned by mod co/i }));
    expect(await screen.findByLabelText(/add user by signup email/i)).toBeInTheDocument();
    // Sole active owner remove button is disabled to prevent 409 LAST_ORG_OWNER_PROTECTED
    expect(screen.getByRole('button', { name: /remove alice@acme\.test/i })).toBeDisabled();
  });

  it('allows platform admins to delegate initial ownership when unchecking include_creator', async () => {
    const user = userEvent.setup();
    organisationService.create.mockResolvedValue({
      id: 'org50',
      name: 'Delegated Org',
      member_count: 1,
      active_member_count: 1,
      members: [],
    });

    renderWithProviders(<OrganisationsPage />, { initialSession: adminSession });

    expect(await screen.findByText('Acme Inc.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /create organisation/i }));

    const dialog = await screen.findByRole('dialog');
    await user.type(within(dialog).getByLabelText(/organisation name/i), 'Delegated Org');
    await user.click(within(dialog).getByLabelText(/add myself as an active owner/i));

    const ownerEmailInput = await within(dialog).findByLabelText(/initial owner email/i);
    await user.type(ownerEmailInput, 'founder@delegated.test');
    await user.click(within(dialog).getByRole('button', { name: /create organisation/i }));

    expect(organisationService.create).toHaveBeenCalledWith({
      name: 'Delegated Org',
      include_creator: false,
      initial_owner_email: 'founder@delegated.test',
    });
  });

  it('disables Leave for a sole active owner even with multiple members and filters members in the roster modal', async () => {
    const user = userEvent.setup();
    organisationService.listMine.mockResolvedValue([
      {
        id: 'org-sole',
        name: 'Sole Owner Co',
        role: 'owner',
        status: 'active',
        member_count: 3,
        active_member_count: 2,
        active_owner_count: 1,
        joined_at: '2026-11-22T12:00:00Z',
        created_at: '2026-11-22T12:00:00Z',
      },
      {
        id: 'org-co',
        name: 'Co-Owned Co',
        role: 'owner',
        status: 'active',
        member_count: 2,
        active_member_count: 2,
        active_owner_count: 2,
        joined_at: '2026-11-22T12:00:00Z',
        created_at: '2026-11-22T12:00:00Z',
      },
    ]);

    organisationService.getById.mockResolvedValue({
      id: 'org-sole',
      name: 'Sole Owner Co',
      member_count: 3,
      active_member_count: 2,
      members: [
        {
          user_id: 'u10',
          email: 'alice@acme.test',
          first_name: 'Alice',
          last_name: 'Owner',
          role: 'owner',
          status: 'active',
          joined_at: '2026-11-22T12:00:00Z',
        },
        {
          user_id: 'u11',
          email: 'bob@acme.test',
          first_name: 'Bob',
          last_name: 'Teammate',
          role: 'member',
          status: 'active',
          joined_at: '2026-11-22T12:00:00Z',
        },
        {
          user_id: 'u12',
          email: 'carol@acme.test',
          role: 'member',
          status: 'invited',
          joined_at: '2026-11-22T12:00:00Z',
        },
      ],
    });

    renderWithProviders(<OrganisationsPage />, { initialSession: orgOwnerSession });

    expect(await screen.findByText('Sole Owner Co')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /leave sole owner co/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /leave co-owned co/i })).toBeEnabled();

    await user.click(screen.getByRole('button', { name: /manage members of sole owner co/i }));
    expect(await screen.findByText('2 active')).toBeInTheDocument();
    expect(screen.getByText('1 invited')).toBeInTheDocument();

    const filterInput = screen.getByLabelText(/^filter members$/i);
    await user.type(filterInput, 'carol');
    expect(screen.getByText('carol@acme.test')).toBeInTheDocument();
    expect(screen.queryByText('bob@acme.test')).not.toBeInTheDocument();
  });

  it('closes the members modal cleanly on self-removal and pre-populates ?search= for admins', async () => {
    const user = userEvent.setup();
    const coOwnedOrg = {
      id: 'org-co',
      name: 'Co-Owned Co',
      role: 'owner',
      status: 'active',
      member_count: 2,
      active_member_count: 2,
      active_owner_count: 2,
      joined_at: '2026-11-22T12:00:00Z',
      created_at: '2026-11-22T12:00:00Z',
    };
    const coOwnedDetails = {
      id: 'org-co',
      name: 'Co-Owned Co',
      member_count: 2,
      active_member_count: 2,
      members: [
        {
          user_id: 'u10',
          email: 'alice@acme.test',
          first_name: 'Alice',
          last_name: 'Owner',
          role: 'owner',
          status: 'active',
          joined_at: '2026-11-22T12:00:00Z',
        },
        {
          user_id: 'u11',
          email: 'bob@acme.test',
          first_name: 'Bob',
          last_name: 'CoOwner',
          role: 'owner',
          status: 'active',
          joined_at: '2026-11-22T12:00:00Z',
        },
      ],
    };
    const afterSelfRemove = {
      ...coOwnedDetails,
      member_count: 1,
      active_member_count: 1,
      members: [coOwnedDetails.members[1]],
    };

    organisationService.listMine.mockResolvedValue([coOwnedOrg]);
    organisationService.getById.mockResolvedValue(coOwnedDetails);
    organisationService.removeMember.mockResolvedValue(afterSelfRemove);

    const { unmount } = renderWithProviders(<OrganisationsPage />, {
      initialSession: orgOwnerSession,
    });

    expect(await screen.findByText('Co-Owned Co')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /manage members of co-owned co/i }));
    expect(await screen.findByLabelText(/add user by signup email/i)).toBeInTheDocument();

    // Removing self as a co-owner prompts confirmation and then closes the modal cleanly
    await user.click(screen.getByRole('button', { name: /remove alice@acme\.test/i }));
    await user.click(await screen.findByRole('button', { name: /^remove member$/i }));
    expect(organisationService.removeMember).toHaveBeenCalledWith('org-co', 'u10');
    expect(screen.queryByLabelText(/add user by signup email/i)).not.toBeInTheDocument();

    unmount();

    // Admin navigating via /organisations?search=Acme pre-populates the search input
    organisationService.listMine.mockResolvedValue([]);
    renderWithProviders(<OrganisationsPage />, {
      initialSession: adminSession,
      initialEntries: ['/organisations?search=Acme'],
    });
    expect(await screen.findByDisplayValue('Acme')).toBeInTheDocument();
  });
});
