import { useState } from 'react';
import {
  ActionIcon,
  Alert,
  Badge,
  Button,
  Divider,
  Group,
  Modal,
  Select,
  Stack,
  Table,
  Text,
  TextInput,
  Tooltip,
} from '@mantine/core';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import { IconLock, IconSearch, IconTrash, IconUserPlus, IconX } from '@tabler/icons-react';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { useAuth } from '../../hooks/useAuth';
import { usePermissions } from '../../hooks/usePermissions';
import { organisationService } from '../../services/organisationService';
import { extractApiError } from '../../utils/errors';
import { capitalize, formatDateTime, getMemberStatusColor } from '../../utils/formatters';

const EMAIL_REGEX = /^\S+@\S+\.\S+$/;

const ADMIN_STATUS_OPTIONS = [
  { value: 'active', label: 'Active' },
  { value: 'invited', label: 'Invited' },
  { value: 'suspended', label: 'Suspended' },
];

const SELF_SERVICE_STATUS_OPTIONS = [
  { value: 'active', label: 'Active' },
  { value: 'suspended', label: 'Suspended' },
];

const ALL_ROLE_OPTIONS = [
  { value: 'owner', label: 'Owner' },
  { value: 'admin', label: 'Admin' },
  { value: 'member', label: 'Member' },
];

const OWNER_INVITE_ROLE_OPTIONS = [
  { value: 'member', label: 'Member' },
  { value: 'admin', label: 'Admin' },
];

const ADMIN_INVITE_ROLE_OPTIONS = [{ value: 'member', label: 'Member' }];

const FILTER_STATUS_OPTIONS = [
  { value: 'all', label: 'All statuses' },
  { value: 'active', label: 'Active' },
  { value: 'invited', label: 'Invited' },
  { value: 'suspended', label: 'Suspended' },
];

export function OrganisationMembersModal({
  opened,
  onClose,
  organisation,
  actorOrgRole = null,
  onUpdated,
}) {
  const { user: currentUser } = useAuth();
  const { hasPermission } = usePermissions();
  const hasMembersLoaded = Array.isArray(organisation?.members);
  const members = hasMembersLoaded ? organisation.members : [];
  const derivedActorMembership = currentUser?.id
    ? members.find((m) => m.user_id === currentUser.id)
    : null;
  const effectiveActorRole =
    currentUser?.id && hasMembersLoaded
      ? derivedActorMembership?.status === 'active'
        ? derivedActorMembership.role
        : null
      : actorOrgRole;

  const hasGlobalManage = hasPermission('organisations.manage_members');
  const hasGlobalRead = hasPermission('organisations.read');
  const isOrgOwner = effectiveActorRole === 'owner';
  const isOrgAdmin = effectiveActorRole === 'admin';
  const canManageMembers = hasGlobalManage || isOrgOwner || isOrgAdmin;

  const activeOwnerCount = members.filter(
    (m) => m.role === 'owner' && m.status === 'active'
  ).length;
  const activeCount = members.filter((m) => m.status === 'active').length;
  const invitedCount = members.filter((m) => m.status === 'invited').length;
  const suspendedCount = members.filter((m) => m.status === 'suspended').length;

  const [emailInput, setEmailInput] = useState('');
  const [emailValidationError, setEmailValidationError] = useState(null);
  const [newRole, setNewRole] = useState('member');
  const [newStatus, setNewStatus] = useState(hasGlobalManage ? 'active' : 'invited');
  const [memberSearch, setMemberSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState('all');
  const [submitting, setSubmitting] = useState(false);
  const [busyUserId, setBusyUserId] = useState(null);
  const [error, setError] = useState(null);

  const normalizedQuery = memberSearch.trim().toLowerCase();
  const filteredMembers = members.filter((member) => {
    if (statusFilter !== 'all' && member.status !== statusFilter) {
      return false;
    }
    if (!normalizedQuery) return true;
    const haystack = [
      member.first_name,
      member.last_name,
      member.email,
      member.role,
      member.user_id,
    ]
      .filter(Boolean)
      .join(' ')
      .toLowerCase();
    return haystack.includes(normalizedQuery);
  });

  const inviteRoleOptions = hasGlobalManage
    ? ALL_ROLE_OPTIONS
    : isOrgOwner
      ? OWNER_INVITE_ROLE_OPTIONS
      : ADMIN_INVITE_ROLE_OPTIONS;

  const handleAddMember = async (event) => {
    event.preventDefault();
    const trimmed = emailInput.trim();
    if (!trimmed || !organisation) return;
    if (!EMAIL_REGEX.test(trimmed)) {
      setEmailValidationError('Enter a valid signup email address');
      return;
    }

    setEmailValidationError(null);
    setError(null);
    setSubmitting(true);
    try {
      const payload = {
        email: trimmed,
        role: newRole,
      };
      if (hasGlobalManage) {
        payload.status = newStatus;
      }
      const updated = await organisationService.addMember(organisation.id, payload);
      setEmailInput('');
      setNewRole('member');
      setNewStatus(hasGlobalManage ? 'active' : 'invited');
      notifications.show({
        title: hasGlobalManage ? 'Member added' : 'Invitation sent',
        message: hasGlobalManage
          ? `Added '${trimmed}' to '${updated.name}'.`
          : `Invited '${trimmed}' to '${updated.name}'. Their profile details stay private until they accept.`,
        color: 'green',
      });
      onUpdated(updated);
    } catch (err) {
      setError(extractApiError(err, 'Failed to add member'));
    } finally {
      setSubmitting(false);
    }
  };

  const handleMemberUpdate = async (memberUserId, patch) => {
    if (!organisation) return;
    setError(null);
    setBusyUserId(memberUserId);
    try {
      const updated = await organisationService.updateMember(organisation.id, memberUserId, patch);
      notifications.show({
        title: 'Member updated',
        message: 'Organisation membership updated successfully.',
        color: 'green',
      });
      onUpdated(updated);
    } catch (err) {
      setError(extractApiError(err, 'Failed to update member'));
    } finally {
      setBusyUserId(null);
    }
  };

  const requestMemberUpdate = (member, patch) => {
    const isSelf = Boolean(currentUser?.id) && member.user_id === currentUser.id;
    const losingOwnOwnerAccess =
      isSelf &&
      !hasGlobalManage &&
      isOrgOwner &&
      ((patch.role && patch.role !== 'owner') || (patch.status && patch.status !== 'active'));

    if (losingOwnOwnerAccess) {
      modals.openConfirmModal({
        title: 'Confirm ownership change',
        centered: true,
        children: (
          <Text size="sm">
            Changing your own membership in <strong>{organisation?.name}</strong> will immediately
            revoke your owner privileges for this organisation. Are you sure you want to continue?
          </Text>
        ),
        labels: { confirm: 'Confirm change', cancel: 'Cancel' },
        confirmProps: { color: 'orange' },
        onConfirm: () => handleMemberUpdate(member.user_id, patch),
      });
      return;
    }

    handleMemberUpdate(member.user_id, patch);
  };

  const executeRemoveMember = async (member) => {
    if (!organisation) return;
    const isInvited = member.status === 'invited';
    const isSelf = Boolean(currentUser?.id) && member.user_id === currentUser.id;
    setError(null);
    setBusyUserId(member.user_id);
    try {
      const updated = await organisationService.removeMember(organisation.id, member.user_id);
      notifications.show({
        title: isInvited ? 'Invitation cancelled' : 'Member removed',
        message: isInvited
          ? `Cancelled invitation for '${member.email || member.user_id}' to '${updated.name}'.`
          : `Removed '${member.email || member.user_id}' from '${updated.name}'.`,
        color: 'green',
      });
      onUpdated(updated);
      if (isSelf && !hasGlobalManage && !hasGlobalRead) {
        onClose();
      }
    } catch (err) {
      setError(
        extractApiError(err, isInvited ? 'Failed to cancel invitation' : 'Failed to remove member')
      );
    } finally {
      setBusyUserId(null);
    }
  };

  const handleRemoveMember = (member) => {
    if (!organisation) return;
    if (member.status === 'invited') {
      executeRemoveMember(member);
      return;
    }
    const label = member.email || member.user_id;
    modals.openConfirmModal({
      title: 'Remove member',
      centered: true,
      children: (
        <Text size="sm">
          Are you sure you want to remove <strong>{label}</strong> from{' '}
          <strong>{organisation.name}</strong>?
        </Text>
      ),
      labels: { confirm: 'Remove member', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: () => executeRemoveMember(member),
    });
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={organisation ? `Members — ${organisation.name}` : 'Organisation Members'}
      size="xl"
    >
      <Stack gap="md">
        <ErrorAlert error={error} onClose={() => setError(null)} />

        {canManageMembers && (
          <>
            {!hasGlobalManage && (
              <Alert color="indigo" variant="light" icon={<IconLock size={16} />} radius="md">
                <Text size="xs">
                  Add users using their signup email address. Invited users must accept before
                  becoming active, and their personal name remains hidden until accepted.
                </Text>
              </Alert>
            )}

            <form onSubmit={handleAddMember}>
              <Group align="flex-end" wrap="wrap" gap="sm">
                <TextInput
                  type="email"
                  label="Add user by signup email"
                  placeholder="user@example.com"
                  value={emailInput}
                  error={emailValidationError}
                  onChange={(e) => {
                    setEmailInput(e.currentTarget.value);
                    if (emailValidationError) setEmailValidationError(null);
                  }}
                  style={{ flex: 1, minWidth: 200 }}
                  required
                />
                <Select
                  label="Org role"
                  data={inviteRoleOptions}
                  value={newRole}
                  allowDeselect={false}
                  onChange={(val) => setNewRole(val || 'member')}
                  w={130}
                />
                {hasGlobalManage && (
                  <Select
                    label="Initial status"
                    data={ADMIN_STATUS_OPTIONS}
                    value={newStatus}
                    allowDeselect={false}
                    onChange={(val) => setNewStatus(val || 'active')}
                    w={135}
                  />
                )}
                <Button type="submit" leftSection={<IconUserPlus size={16} />} loading={submitting}>
                  {hasGlobalManage ? 'Add member' : 'Invite user'}
                </Button>
              </Group>
            </form>
            <Divider />
          </>
        )}

        {members.length === 0 ? (
          <Text size="sm" c="dimmed" ta="center" py="md">
            This organisation currently has no members.
          </Text>
        ) : (
          <Stack gap="sm">
            <Group justify="space-between" align="center" wrap="wrap" gap="sm">
              <Group gap="xs">
                <Badge variant="light" color="green">
                  {activeCount} active
                </Badge>
                <Badge variant="light" color="yellow">
                  {invitedCount} invited
                </Badge>
                <Badge variant="light" color="red">
                  {suspendedCount} suspended
                </Badge>
              </Group>
              <Group gap="xs" wrap="wrap">
                <TextInput
                  size="xs"
                  placeholder="Filter members..."
                  aria-label="Filter members"
                  leftSection={<IconSearch size={14} />}
                  value={memberSearch}
                  onChange={(e) => setMemberSearch(e.currentTarget.value)}
                  w={200}
                />
                <Select
                  size="xs"
                  aria-label="Filter members by status"
                  data={FILTER_STATUS_OPTIONS}
                  value={statusFilter}
                  allowDeselect={false}
                  onChange={(val) => setStatusFilter(val || 'all')}
                  w={140}
                />
              </Group>
            </Group>

            {filteredMembers.length === 0 ? (
              <Text size="sm" c="dimmed" ta="center" py="md">
                No members match your current filter.
              </Text>
            ) : (
              <Table.ScrollContainer minWidth={620}>
                <Table striped highlightOnHover verticalSpacing="xs">
                  <Table.Thead>
                    <Table.Tr>
                      <Table.Th>Member</Table.Th>
                      <Table.Th>Role</Table.Th>
                      <Table.Th>Status</Table.Th>
                      <Table.Th>Joined</Table.Th>
                      {canManageMembers && <Table.Th ta="right">Actions</Table.Th>}
                    </Table.Tr>
                  </Table.Thead>
                  <Table.Tbody>
                    {filteredMembers.map((member) => {
                      const fullName =
                        `${member.first_name || ''} ${member.last_name || ''}`.trim();
                      const canEditRole = hasGlobalManage || isOrgOwner;
                      const canEditTarget =
                        hasGlobalManage || isOrgOwner || (isOrgAdmin && member.role === 'member');
                      const isPendingSelfService = !hasGlobalManage && member.status === 'invited';
                      const isLastActiveOwner =
                        member.role === 'owner' &&
                        member.status === 'active' &&
                        activeOwnerCount <= 1;
                      const roleOptionsForMember =
                        !hasGlobalManage && member.status !== 'active' && member.role !== 'owner'
                          ? OWNER_INVITE_ROLE_OPTIONS
                          : ALL_ROLE_OPTIONS;

                      return (
                        <Table.Tr key={member.user_id}>
                          <Table.Td>
                            <Text size="sm" fw={500}>
                              {fullName || member.email || member.user_id}
                            </Text>
                            {fullName && member.email && (
                              <Text size="xs" c="dimmed">
                                {member.email}
                              </Text>
                            )}
                            {!fullName && member.status === 'invited' && (
                              <Text size="xs" c="dimmed">
                                Name hidden until invitation is accepted
                              </Text>
                            )}
                          </Table.Td>
                          <Table.Td>
                            {canEditRole ? (
                              <Tooltip
                                label="Assign another active owner before demoting the last owner"
                                disabled={!isLastActiveOwner}
                              >
                                <div>
                                  <Select
                                    aria-label={`Role for ${member.email || member.user_id}`}
                                    size="xs"
                                    w={115}
                                    data={roleOptionsForMember}
                                    value={member.role || 'member'}
                                    allowDeselect={false}
                                    disabled={busyUserId === member.user_id || isLastActiveOwner}
                                    onChange={(val) => {
                                      if (val && val !== (member.role || 'member')) {
                                        requestMemberUpdate(member, { role: val });
                                      }
                                    }}
                                  />
                                </div>
                              </Tooltip>
                            ) : (
                              <Badge variant="light" color="indigo">
                                {capitalize(member.role || 'member')}
                              </Badge>
                            )}
                          </Table.Td>
                          <Table.Td>
                            {canEditTarget && !isPendingSelfService ? (
                              <Tooltip
                                label="Assign another active owner before suspending the last owner"
                                disabled={!isLastActiveOwner}
                              >
                                <div>
                                  <Select
                                    aria-label={`Status for ${member.email || member.user_id}`}
                                    size="xs"
                                    w={130}
                                    data={
                                      hasGlobalManage
                                        ? ADMIN_STATUS_OPTIONS
                                        : SELF_SERVICE_STATUS_OPTIONS
                                    }
                                    value={member.status}
                                    allowDeselect={false}
                                    disabled={busyUserId === member.user_id || isLastActiveOwner}
                                    onChange={(val) => {
                                      if (val && val !== member.status) {
                                        requestMemberUpdate(member, { status: val });
                                      }
                                    }}
                                  />
                                </div>
                              </Tooltip>
                            ) : (
                              <Badge variant="light" color={getMemberStatusColor(member.status)}>
                                {capitalize(member.status)}
                              </Badge>
                            )}
                          </Table.Td>
                          <Table.Td>
                            <Text size="xs" c="dimmed">
                              {formatDateTime(member.joined_at)}
                            </Text>
                          </Table.Td>
                          {canManageMembers && (
                            <Table.Td>
                              <Group justify="flex-end">
                                {canEditTarget &&
                                  (member.status === 'invited' ? (
                                    <Button
                                      size="xs"
                                      variant="subtle"
                                      color="red"
                                      leftSection={<IconX size={14} />}
                                      loading={busyUserId === member.user_id}
                                      aria-label={`Cancel invitation for ${member.email || member.user_id}`}
                                      onClick={() => handleRemoveMember(member)}
                                    >
                                      Cancel invite
                                    </Button>
                                  ) : (
                                    <Tooltip
                                      label={
                                        isLastActiveOwner
                                          ? 'Assign another active owner before removing the last owner'
                                          : 'Remove member'
                                      }
                                    >
                                      <ActionIcon
                                        variant="subtle"
                                        color="red"
                                        loading={busyUserId === member.user_id}
                                        disabled={isLastActiveOwner}
                                        aria-label={`Remove ${member.email || member.user_id}`}
                                        onClick={() => handleRemoveMember(member)}
                                      >
                                        <IconTrash size={16} />
                                      </ActionIcon>
                                    </Tooltip>
                                  ))}
                              </Group>
                            </Table.Td>
                          )}
                        </Table.Tr>
                      );
                    })}
                  </Table.Tbody>
                </Table>
              </Table.ScrollContainer>
            )}
          </Stack>
        )}
      </Stack>
    </Modal>
  );
}
