import { useCallback, useEffect, useState } from 'react';
import {
  ActionIcon,
  Badge,
  Button,
  Group,
  Menu,
  Pagination,
  Paper,
  Select,
  Skeleton,
  Stack,
  Table,
  Text,
  TextInput,
  Tooltip,
} from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import {
  IconDotsVertical,
  IconEdit,
  IconEye,
  IconPlus,
  IconSearch,
  IconShield,
  IconTrash,
  IconUserCheck,
  IconUserOff,
} from '@tabler/icons-react';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { PageHeader } from '../../components/common/PageHeader';
import { PermissionGuard } from '../../components/common/PermissionGuard';
import { StatusBadge } from '../../components/common/StatusBadge';
import { useAuth } from '../../hooks/useAuth';
import { usePermissions } from '../../hooks/usePermissions';
import { roleService } from '../../services/roleService';
import { userService } from '../../services/userService';
import { extractApiError } from '../../utils/errors';
import { formatDateTime, formatUserName } from '../../utils/formatters';
import { AssignRolesModal } from './AssignRolesModal';
import { UserDetailDrawer } from './UserDetailDrawer';
import { UserFormModal } from './UserFormModal';

const STATUS_OPTIONS = [
  { value: '', label: 'All statuses' },
  { value: 'active', label: 'Active' },
  { value: 'disabled', label: 'Disabled' },
  { value: 'locked', label: 'Locked' },
  { value: 'pending_verification', label: 'Pending Verification' },
];

export function UsersPage() {
  const { user: currentUser } = useAuth();
  const { hasPermission } = usePermissions();

  const [usersData, setUsersData] = useState({ items: [], total: 0, page: 1, total_pages: 1 });
  const [roles, setRoles] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [reloadTick, setReloadTick] = useState(0);

  const [search, setSearch] = useState('');
  const [debouncedSearch] = useDebouncedValue(search, 300);
  const [statusFilter, setStatusFilter] = useState('');
  const [page, setPage] = useState(1);

  const [formOpen, setFormOpen] = useState(false);
  const [editingUser, setEditingUser] = useState(null);
  const [detailUserId, setDetailUserId] = useState(null);
  const [rolesModalUser, setRolesModalUser] = useState(null);

  const triggerReload = useCallback(() => setReloadTick((n) => n + 1), []);

  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const data = await userService.list({
          page,
          page_size: 10,
          search: debouncedSearch,
          status: statusFilter,
        });
        if (active) {
          setUsersData(data);
          setError(null);
        }
      } catch (err) {
        if (active) setError(extractApiError(err, 'Failed to load users'));
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [page, debouncedSearch, statusFilter, reloadTick]);

  useEffect(() => {
    if (!hasPermission('roles.read')) return;
    let active = true;
    roleService
      .list({ page: 1, page_size: 100 })
      .then((res) => {
        if (active) setRoles(res.items || []);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [hasPermission]);

  const handleToggleStatus = async (targetUser) => {
    const nextStatus = targetUser.status === 'active' ? 'disabled' : 'active';
    try {
      await userService.updateStatus(targetUser.id, nextStatus);
      notifications.show({
        title: 'Status updated',
        message: `${targetUser.email} is now ${nextStatus}.`,
        color: 'green',
      });
      triggerReload();
    } catch (err) {
      setError(extractApiError(err, 'Failed to update user status'));
    }
  };

  const confirmDelete = (targetUser) => {
    modals.openConfirmModal({
      title: 'Delete user account',
      centered: true,
      children: (
        <Text size="sm">
          Are you sure you want to permanently delete <strong>{targetUser.email}</strong>? This
          action cannot be undone and revokes all active sessions.
        </Text>
      ),
      labels: { confirm: 'Delete user', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: async () => {
        try {
          await userService.remove(targetUser.id);
          notifications.show({
            title: 'User deleted',
            message: `${targetUser.email} has been deleted.`,
            color: 'green',
          });
          triggerReload();
        } catch (err) {
          setError(extractApiError(err, 'Failed to delete user'));
        }
      },
    });
  };

  return (
    <Stack gap="md">
      <PageHeader
        title="User Management"
        description="View accounts, manage statuses, assign roles, and inspect effective permissions"
        actions={
          <PermissionGuard permission="users.create">
            <Button
              leftSection={<IconPlus size={16} />}
              onClick={() => {
                setEditingUser(null);
                setFormOpen(true);
              }}
            >
              Create User
            </Button>
          </PermissionGuard>
        }
      />

      <ErrorAlert error={error} onClose={() => setError(null)} />

      <Paper withBorder p="md" radius="md">
        <Group mb="md" grow>
          <TextInput
            placeholder="Search by name or email..."
            leftSection={<IconSearch size={16} />}
            value={search}
            onChange={(e) => {
              setSearch(e.currentTarget.value);
              setPage(1);
            }}
          />
          <Select
            placeholder="Filter by status"
            data={STATUS_OPTIONS}
            value={statusFilter}
            onChange={(val) => {
              setStatusFilter(val || '');
              setPage(1);
            }}
            clearable
          />
        </Group>

        {loading ? (
          <Stack gap="xs">
            {Array.from({ length: 5 }).map((_, i) => (
              <Skeleton key={i} height={42} />
            ))}
          </Stack>
        ) : (
          <Table.ScrollContainer minWidth={720}>
            <Table striped highlightOnHover verticalSpacing="sm">
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Name</Table.Th>
                  <Table.Th>Email</Table.Th>
                  <Table.Th>Status</Table.Th>
                  <Table.Th>Roles</Table.Th>
                  <Table.Th>Last Login</Table.Th>
                  <Table.Th ta="right">Actions</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {usersData.items.length === 0 ? (
                  <Table.Tr>
                    <Table.Td colSpan={6}>
                      <Text ta="center" c="dimmed" py="md">
                        No users found matching your criteria.
                      </Text>
                    </Table.Td>
                  </Table.Tr>
                ) : (
                  usersData.items.map((u) => {
                    const isSelf = currentUser?.id === u.id;
                    return (
                      <Table.Tr key={u.id}>
                        <Table.Td fw={500}>{formatUserName(u)}</Table.Td>
                        <Table.Td>{u.email}</Table.Td>
                        <Table.Td>
                          <StatusBadge status={u.status} />
                        </Table.Td>
                        <Table.Td>
                          <Group gap={4}>
                            {(u.roles || []).map((role) => (
                              <Badge key={role.id} size="sm" variant="light" color="indigo">
                                {role.name}
                              </Badge>
                            ))}
                          </Group>
                        </Table.Td>
                        <Table.Td>
                          <Text size="xs" c="dimmed">
                            {formatDateTime(u.last_login_at)}
                          </Text>
                        </Table.Td>
                        <Table.Td>
                          <Group gap={4} justify="flex-end" wrap="nowrap">
                            <Tooltip label="View details">
                              <ActionIcon
                                variant="subtle"
                                aria-label={`View ${u.email}`}
                                onClick={() => setDetailUserId(u.id)}
                              >
                                <IconEye size={16} />
                              </ActionIcon>
                            </Tooltip>

                            <Menu shadow="md" position="bottom-end">
                              <Menu.Target>
                                <ActionIcon variant="subtle" aria-label={`Actions for ${u.email}`}>
                                  <IconDotsVertical size={16} />
                                </ActionIcon>
                              </Menu.Target>
                              <Menu.Dropdown>
                                <Menu.Item
                                  leftSection={<IconEye size={15} />}
                                  onClick={() => setDetailUserId(u.id)}
                                >
                                  View details &amp; permissions
                                </Menu.Item>

                                {hasPermission('users.update') && (
                                  <Menu.Item
                                    leftSection={<IconEdit size={15} />}
                                    onClick={() => {
                                      setEditingUser(u);
                                      setFormOpen(true);
                                    }}
                                  >
                                    Edit user
                                  </Menu.Item>
                                )}

                                {hasPermission('users.assign_roles') && (
                                  <Menu.Item
                                    leftSection={<IconShield size={15} />}
                                    onClick={() => setRolesModalUser(u)}
                                  >
                                    Assign roles
                                  </Menu.Item>
                                )}

                                {hasPermission('users.update') && !isSelf && (
                                  <Menu.Item
                                    leftSection={
                                      u.status === 'active' ? (
                                        <IconUserOff size={15} />
                                      ) : (
                                        <IconUserCheck size={15} />
                                      )
                                    }
                                    color={u.status === 'active' ? 'orange' : 'green'}
                                    onClick={() => handleToggleStatus(u)}
                                  >
                                    {u.status === 'active' ? 'Disable account' : 'Enable account'}
                                  </Menu.Item>
                                )}

                                {hasPermission('users.delete') && !isSelf && (
                                  <>
                                    <Menu.Divider />
                                    <Menu.Item
                                      color="red"
                                      leftSection={<IconTrash size={15} />}
                                      onClick={() => confirmDelete(u)}
                                    >
                                      Delete user
                                    </Menu.Item>
                                  </>
                                )}
                              </Menu.Dropdown>
                            </Menu>
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

        <Group justify="space-between" mt="md">
          <Text size="sm" c="dimmed">
            Showing {usersData.items.length} of {usersData.total} users
          </Text>
          <Pagination
            total={Math.max(usersData.total_pages || 1, 1)}
            value={page}
            onChange={setPage}
          />
        </Group>
      </Paper>

      <UserFormModal
        key={`${editingUser?.id || 'new'}-${formOpen}`}
        opened={formOpen}
        onClose={() => setFormOpen(false)}
        user={editingUser}
        availableRoles={roles}
        canAssignRoles={hasPermission('users.assign_roles')}
        onSaved={() => {
          setFormOpen(false);
          triggerReload();
        }}
      />

      <UserDetailDrawer userId={detailUserId} onClose={() => setDetailUserId(null)} />

      <AssignRolesModal
        key={rolesModalUser?.id || 'none'}
        user={rolesModalUser}
        availableRoles={roles}
        onClose={() => setRolesModalUser(null)}
        onSaved={() => {
          setRolesModalUser(null);
          triggerReload();
        }}
      />
    </Stack>
  );
}
