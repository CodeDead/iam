import { useCallback, useEffect, useRef, useState } from 'react';
import {
  ActionIcon,
  Alert,
  Badge,
  Button,
  Group,
  Pagination,
  Paper,
  Skeleton,
  Stack,
  Table,
  Text,
  TextInput,
  Title,
  Tooltip,
} from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import {
  IconBuildingPlus,
  IconCheck,
  IconEdit,
  IconLogout,
  IconMail,
  IconSearch,
  IconTrash,
  IconUsers,
  IconX,
} from '@tabler/icons-react';
import { useSearchParams } from 'react-router-dom';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { PageHeader } from '../../components/common/PageHeader';
import { useAuth } from '../../hooks/useAuth';
import { usePermissions } from '../../hooks/usePermissions';
import { organisationService } from '../../services/organisationService';
import { extractApiError } from '../../utils/errors';
import { capitalize, formatDateTime, getMemberStatusColor } from '../../utils/formatters';
import { OrganisationFormModal } from './OrganisationFormModal';
import { OrganisationMembersModal } from './OrganisationMembersModal';

export function OrganisationsPage() {
  const { user: currentUser } = useAuth();
  const { hasPermission } = usePermissions();
  const [searchParams] = useSearchParams();
  const canReadAllOrgs = hasPermission('organisations.read');
  const canGlobalManageMembers = hasPermission('organisations.manage_members');
  const canGlobalUpdate = hasPermission('organisations.update');
  const canGlobalDelete = hasPermission('organisations.delete');

  const [myOrgs, setMyOrgs] = useState([]);
  const [data, setData] = useState({ items: [], total: 0, page: 1, total_pages: 1 });
  const [loadingMine, setLoadingMine] = useState(true);
  const [loadingAll, setLoadingAll] = useState(canReadAllOrgs);
  const [error, setError] = useState(null);
  const [search, setSearch] = useState(() => searchParams.get('search') || '');
  const [debouncedSearch] = useDebouncedValue(search, 300);
  const [page, setPage] = useState(1);
  const [reloadTick, setReloadTick] = useState(0);
  const [busyActions, setBusyActions] = useState({});
  const openMembersSeqRef = useRef(0);
  const isMountedRef = useRef(true);

  const [formOpen, setFormOpen] = useState(false);
  const [editingOrg, setEditingOrg] = useState(null);
  const [membersModalState, setMembersModalState] = useState({ org: null, actorRole: null });

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  const setOrgActionBusy = useCallback((orgId, action) => {
    setBusyActions((prev) => {
      if (!action) {
        const next = { ...prev };
        delete next[orgId];
        return next;
      }
      return { ...prev, [orgId]: action };
    });
  }, []);

  const triggerReload = useCallback(() => setReloadTick((n) => n + 1), []);

  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const mine = await organisationService.listMine();
        if (active) {
          setMyOrgs(mine || []);
        }
      } catch (err) {
        if (active) setError(extractApiError(err, 'Failed to load your organisations'));
      } finally {
        if (active) setLoadingMine(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [reloadTick]);

  useEffect(() => {
    if (!canReadAllOrgs) {
      return;
    }
    let active = true;
    (async () => {
      try {
        const globalList = await organisationService.list({
          page,
          page_size: 15,
          search: debouncedSearch,
        });
        if (active) {
          if (page > 1 && globalList.total_pages < page) {
            setPage(Math.max(1, globalList.total_pages));
            return;
          }
          setData(globalList);
          setError(null);
        }
      } catch (err) {
        if (active) setError(extractApiError(err, 'Failed to load organisations'));
      } finally {
        if (active) setLoadingAll(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [canReadAllOrgs, page, debouncedSearch, reloadTick]);

  const openMembersForSummary = async (summary) => {
    const seq = ++openMembersSeqRef.current;
    setError(null);
    setOrgActionBusy(summary.id, 'members');
    try {
      const fullOrg = await organisationService.getById(summary.id);
      if (isMountedRef.current && seq === openMembersSeqRef.current) {
        setMembersModalState({ org: fullOrg, actorRole: summary.role });
      }
    } catch (err) {
      if (isMountedRef.current && seq === openMembersSeqRef.current) {
        setError(extractApiError(err, 'Failed to load organisation members'));
      }
    } finally {
      if (isMountedRef.current) {
        setOrgActionBusy(summary.id, null);
      }
    }
  };

  const handleAcceptInvite = async (org) => {
    setError(null);
    setOrgActionBusy(org.id, 'accept');
    try {
      await organisationService.acceptInvitation(org.id);
      notifications.show({
        title: 'Invitation accepted',
        message: `You are now an active member of '${org.name}'.`,
        color: 'green',
      });
      triggerReload();
    } catch (err) {
      setError(extractApiError(err, 'Failed to accept invitation'));
    } finally {
      if (isMountedRef.current) {
        setOrgActionBusy(org.id, null);
      }
    }
  };

  const handleLeaveOrDecline = (org, isInvite = false) => {
    modals.openConfirmModal({
      title: isInvite ? 'Decline invitation' : 'Leave organisation',
      children: (
        <Text size="sm">
          Are you sure you want to {isInvite ? 'decline the invitation to' : 'leave'}{' '}
          <strong>{org.name}</strong>?
        </Text>
      ),
      labels: { confirm: isInvite ? 'Decline' : 'Leave', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: async () => {
        setError(null);
        setOrgActionBusy(org.id, isInvite ? 'decline' : 'leave');
        try {
          await organisationService.leave(org.id);
          notifications.show({
            title: isInvite ? 'Invitation declined' : 'Left organisation',
            message: isInvite
              ? `Declined invitation to '${org.name}'.`
              : `You have left '${org.name}'.`,
            color: 'green',
          });
          triggerReload();
        } catch (err) {
          const apiError = extractApiError(err, 'Failed to update membership');
          setError(apiError);
          notifications.show({
            title: 'Action failed',
            message: apiError.message,
            color: 'red',
          });
        } finally {
          if (isMountedRef.current) {
            setOrgActionBusy(org.id, null);
          }
        }
      },
    });
  };

  const confirmDelete = (org) => {
    modals.openConfirmModal({
      title: 'Delete organisation',
      children: (
        <Text size="sm">
          Are you sure you want to permanently delete <strong>{org.name}</strong>? All membership
          associations will be removed.
        </Text>
      ),
      labels: { confirm: 'Delete', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: async () => {
        setError(null);
        setOrgActionBusy(org.id, 'delete');
        try {
          await organisationService.remove(org.id);
          notifications.show({
            title: 'Organisation deleted',
            message: `Deleted organisation '${org.name}'.`,
            color: 'green',
          });
          triggerReload();
        } catch (err) {
          const apiError = extractApiError(err, 'Failed to delete organisation');
          setError(apiError);
          notifications.show({
            title: 'Unable to delete organisation',
            message: apiError.message,
            color: 'red',
          });
        } finally {
          if (isMountedRef.current) {
            setOrgActionBusy(org.id, null);
          }
        }
      },
    });
  };

  const pendingInvitations = myOrgs.filter((org) => org.status === 'invited');
  const activeOrSuspendedMyOrgs = myOrgs.filter((org) => org.status !== 'invited');

  return (
    <Stack gap="md">
      <PageHeader
        title="Organisations"
        description={
          canReadAllOrgs
            ? 'Manage organisation accounts, memberships, and invitations'
            : 'Manage your organisations, invite members by signup email, and respond to invitations'
        }
        actions={
          <Button
            leftSection={<IconBuildingPlus size={16} />}
            onClick={() => {
              setEditingOrg(null);
              setFormOpen(true);
            }}
          >
            Create Organisation
          </Button>
        }
      />

      <ErrorAlert error={error} onClose={() => setError(null)} />

      {pendingInvitations.length > 0 && (
        <Alert
          color="indigo"
          variant="light"
          radius="md"
          icon={<IconMail size={20} />}
          title={`Pending Organisation ${pendingInvitations.length === 1 ? 'Invitation' : 'Invitations'}`}
        >
          <Stack gap="xs" mt="xs">
            {pendingInvitations.map((invite) => (
              <Group key={invite.id} justify="space-between" align="center" wrap="wrap" gap="sm">
                <Group gap="xs" wrap="wrap">
                  <Text size="sm">
                    You have been invited to join <strong>{invite.name}</strong> as
                  </Text>
                  <Badge size="xs" variant="light" color="indigo">
                    {capitalize(invite.role || 'member')}
                  </Badge>
                </Group>
                <Group gap="xs">
                  <Button
                    size="xs"
                    color="green"
                    leftSection={<IconCheck size={14} />}
                    loading={busyActions[invite.id] === 'accept'}
                    disabled={Boolean(busyActions[invite.id])}
                    onClick={() => handleAcceptInvite(invite)}
                  >
                    Accept
                  </Button>
                  <Button
                    size="xs"
                    variant="default"
                    color="red"
                    leftSection={<IconX size={14} />}
                    loading={busyActions[invite.id] === 'decline'}
                    disabled={Boolean(busyActions[invite.id])}
                    onClick={() => handleLeaveOrDecline(invite, true)}
                  >
                    Decline
                  </Button>
                </Group>
              </Group>
            ))}
          </Stack>
        </Alert>
      )}

      {(!canReadAllOrgs || activeOrSuspendedMyOrgs.length > 0) && (
        <Paper withBorder p="md" radius="md">
          <Group justify="space-between" mb="sm">
            <div>
              <Title order={4}>My Organisations</Title>
              <Text size="xs" c="dimmed">
                Organisations you belong to — users may belong to multiple organisations
              </Text>
            </div>
            <Badge variant="light" color="grape">
              {activeOrSuspendedMyOrgs.length}{' '}
              {activeOrSuspendedMyOrgs.length === 1 ? 'organisation' : 'organisations'}
            </Badge>
          </Group>

          {loadingMine ? (
            <Stack gap="xs">
              <Skeleton height={44} />
            </Stack>
          ) : activeOrSuspendedMyOrgs.length === 0 ? (
            <Text ta="center" c="dimmed" py="md" size="sm">
              You are not currently a member of any organisation.
            </Text>
          ) : (
            <Table.ScrollContainer minWidth={560}>
              <Table striped highlightOnHover verticalSpacing="sm">
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Organisation</Table.Th>
                    <Table.Th>Your Role</Table.Th>
                    <Table.Th>Status</Table.Th>
                    <Table.Th>Members</Table.Th>
                    <Table.Th ta="right">Actions</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {activeOrSuspendedMyOrgs.map((org) => {
                    const canManage =
                      org.status === 'active' && (org.role === 'owner' || org.role === 'admin');
                    const isOwner = org.status === 'active' && org.role === 'owner';
                    const activeOwnerCount =
                      typeof org.active_owner_count === 'number'
                        ? org.active_owner_count
                        : isOwner
                          ? 1
                          : 0;
                    const isSoleActiveOwner = isOwner && activeOwnerCount <= 1;

                    return (
                      <Table.Tr key={org.id}>
                        <Table.Td>
                          <Text size="sm" fw={600}>
                            {org.name}
                          </Text>
                        </Table.Td>
                        <Table.Td>
                          <Badge variant="light" color="indigo">
                            {capitalize(org.role || 'member')}
                          </Badge>
                        </Table.Td>
                        <Table.Td>
                          <Badge variant="light" color={getMemberStatusColor(org.status)}>
                            {capitalize(org.status)}
                          </Badge>
                        </Table.Td>
                        <Table.Td>
                          <Text size="sm">
                            {typeof org.active_member_count === 'number'
                              ? `${org.active_member_count} active / ${org.member_count} total`
                              : org.member_count}
                          </Text>
                        </Table.Td>
                        <Table.Td>
                          <Group justify="flex-end" gap="xs" wrap="nowrap">
                            {canManage && (
                              <Button
                                size="xs"
                                variant="light"
                                color="indigo"
                                leftSection={<IconUsers size={14} />}
                                loading={busyActions[org.id] === 'members'}
                                disabled={Boolean(busyActions[org.id])}
                                aria-label={`Manage members of ${org.name}`}
                                onClick={() => openMembersForSummary(org)}
                              >
                                Members
                              </Button>
                            )}
                            {isOwner && (
                              <Tooltip label="Rename organisation">
                                <ActionIcon
                                  variant="subtle"
                                  disabled={Boolean(busyActions[org.id])}
                                  aria-label={`Edit ${org.name}`}
                                  onClick={() => {
                                    setEditingOrg(org);
                                    setFormOpen(true);
                                  }}
                                >
                                  <IconEdit size={16} />
                                </ActionIcon>
                              </Tooltip>
                            )}
                            {isOwner && (
                              <Tooltip label="Delete organisation">
                                <ActionIcon
                                  variant="subtle"
                                  color="red"
                                  loading={busyActions[org.id] === 'delete'}
                                  disabled={Boolean(busyActions[org.id])}
                                  aria-label={`Delete ${org.name}`}
                                  onClick={() => confirmDelete(org)}
                                >
                                  <IconTrash size={16} />
                                </ActionIcon>
                              </Tooltip>
                            )}
                            <Tooltip
                              label={
                                isSoleActiveOwner
                                  ? 'Promote another active member to Owner before leaving, or delete the organisation'
                                  : 'Leave organisation'
                              }
                            >
                              <ActionIcon
                                variant="subtle"
                                color="red"
                                loading={busyActions[org.id] === 'leave'}
                                disabled={isSoleActiveOwner || Boolean(busyActions[org.id])}
                                aria-label={`Leave ${org.name}`}
                                onClick={() => handleLeaveOrDecline(org, false)}
                              >
                                <IconLogout size={16} />
                              </ActionIcon>
                            </Tooltip>
                          </Group>
                        </Table.Td>
                      </Table.Tr>
                    );
                  })}
                </Table.Tbody>
              </Table>
            </Table.ScrollContainer>
          )}
        </Paper>
      )}

      {canReadAllOrgs && (
        <Paper withBorder p="md" radius="md">
          <Group justify="space-between" mb="md" wrap="wrap" gap="sm">
            <div>
              <Title order={4}>All Organisations</Title>
              <Text size="xs" c="dimmed">
                Platform-wide organisation directory
              </Text>
            </div>
            <Group gap="sm" style={{ flex: 1, justifyContent: 'flex-end' }}>
              <TextInput
                placeholder="Search organisations by name..."
                leftSection={<IconSearch size={16} />}
                value={search}
                onChange={(e) => {
                  setSearch(e.currentTarget.value);
                  setPage(1);
                }}
                style={{ flex: 1, maxWidth: 320 }}
              />
              <Text size="xs" c="dimmed">
                {data.total} {data.total === 1 ? 'organisation' : 'organisations'}
              </Text>
            </Group>
          </Group>

          {loadingAll ? (
            <Stack gap="xs">
              {Array.from({ length: 4 }).map((_, i) => (
                <Skeleton key={i} height={44} />
              ))}
            </Stack>
          ) : (
            <Table.ScrollContainer minWidth={640}>
              <Table striped highlightOnHover verticalSpacing="sm">
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Organisation</Table.Th>
                    <Table.Th>Members</Table.Th>
                    <Table.Th>Created</Table.Th>
                    <Table.Th ta="right">Actions</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {data.items.length === 0 ? (
                    <Table.Tr>
                      <Table.Td colSpan={4}>
                        <Text ta="center" c="dimmed" py="lg">
                          No organisations found.
                        </Text>
                      </Table.Td>
                    </Table.Tr>
                  ) : (
                    data.items.map((org) => {
                      const ownMembership = currentUser?.id
                        ? (org.members || []).find(
                            (m) => m.user_id === currentUser.id && m.status === 'active'
                          )
                        : null;
                      const actorRole = ownMembership?.role || null;
                      const canManageRow =
                        canGlobalManageMembers || actorRole === 'owner' || actorRole === 'admin';
                      const canEditRow = canGlobalUpdate || actorRole === 'owner';
                      const canDeleteRow = canGlobalDelete || actorRole === 'owner';

                      return (
                        <Table.Tr key={org.id}>
                          <Table.Td>
                            <Text size="sm" fw={600}>
                              {org.name}
                            </Text>
                          </Table.Td>
                          <Table.Td>
                            <Group gap="xs">
                              <Badge variant="light" color="green">
                                {org.active_member_count} active
                              </Badge>
                              <Badge variant="light" color="gray">
                                {org.member_count} total
                              </Badge>
                            </Group>
                          </Table.Td>
                          <Table.Td>
                            <Text size="xs" c="dimmed">
                              {formatDateTime(org.created_at)}
                            </Text>
                          </Table.Td>
                          <Table.Td>
                            <Group justify="flex-end" gap={4}>
                              <Tooltip
                                label={canManageRow ? 'View & manage members' : 'View members'}
                              >
                                <ActionIcon
                                  variant="subtle"
                                  color="indigo"
                                  disabled={Boolean(busyActions[org.id])}
                                  aria-label={
                                    canManageRow
                                      ? `Manage members of ${org.name}`
                                      : `View members of ${org.name}`
                                  }
                                  onClick={() => setMembersModalState({ org, actorRole })}
                                >
                                  <IconUsers size={16} />
                                </ActionIcon>
                              </Tooltip>
                              {canEditRow && (
                                <Tooltip label="Edit organisation">
                                  <ActionIcon
                                    variant="subtle"
                                    disabled={Boolean(busyActions[org.id])}
                                    aria-label={`Edit ${org.name}`}
                                    onClick={() => {
                                      setEditingOrg(org);
                                      setFormOpen(true);
                                    }}
                                  >
                                    <IconEdit size={16} />
                                  </ActionIcon>
                                </Tooltip>
                              )}
                              {canDeleteRow && (
                                <Tooltip label="Delete organisation">
                                  <ActionIcon
                                    variant="subtle"
                                    color="red"
                                    loading={busyActions[org.id] === 'delete'}
                                    disabled={Boolean(busyActions[org.id])}
                                    aria-label={`Delete ${org.name}`}
                                    onClick={() => confirmDelete(org)}
                                  >
                                    <IconTrash size={16} />
                                  </ActionIcon>
                                </Tooltip>
                              )}
                            </Group>
                          </Table.Td>
                        </Table.Tr>
                      );
                    })
                  )}
                </Table.Tbody>
              </Table>
            </Table.ScrollContainer>
          )}

          {data.total_pages > 1 && (
            <Group justify="flex-end" mt="md">
              <Pagination total={data.total_pages} value={page} onChange={setPage} />
            </Group>
          )}
        </Paper>
      )}

      <OrganisationFormModal
        key={editingOrg ? `edit-${editingOrg.id}-${formOpen}` : `create-${formOpen}`}
        opened={formOpen}
        onClose={() => setFormOpen(false)}
        organisation={editingOrg}
        onSaved={triggerReload}
      />

      <OrganisationMembersModal
        key={membersModalState.org ? `members-${membersModalState.org.id}` : 'members-closed'}
        opened={Boolean(membersModalState.org)}
        onClose={() => setMembersModalState({ org: null, actorRole: null })}
        organisation={membersModalState.org}
        actorOrgRole={membersModalState.actorRole}
        onUpdated={(updated) => {
          setMembersModalState((prev) => ({ ...prev, org: updated }));
          triggerReload();
        }}
      />
    </Stack>
  );
}
