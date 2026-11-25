import { useCallback, useEffect, useState } from 'react';
import {
  ActionIcon,
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
  Tooltip,
} from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import { IconEdit, IconEye, IconPlus, IconSearch, IconTrash } from '@tabler/icons-react';
import ErrorAlert from '../../components/common/ErrorAlert';
import PageHeader from '../../components/common/PageHeader';
import PermissionGuard from '../../components/common/PermissionGuard';
import { usePermissions } from '../../hooks/usePermissions';
import { permissionService } from '../../services/permissionService';
import { roleService } from '../../services/roleService';
import { extractApiError } from '../../utils/errors';
import { fetchAllPages } from '../../utils/pagination';
import RoleDetailModal from './RoleDetailModal';
import RoleFormModal from './RoleFormModal';

const RolesPage = () => {
  const { hasPermission } = usePermissions();
  const [rolesData, setRolesData] = useState({ items: [], total: 0, page: 1, total_pages: 1 });
  const [allPermissions, setAllPermissions] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [reloadTick, setReloadTick] = useState(0);

  const [search, setSearch] = useState('');
  const [debouncedSearch] = useDebouncedValue(search, 300);
  const [page, setPage] = useState(1);

  const [formOpen, setFormOpen] = useState(false);
  const [editingRole, setEditingRole] = useState(null);
  const [detailRoleId, setDetailRoleId] = useState(null);

  const triggerReload = useCallback(() => setReloadTick((n) => n + 1), []);

  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const data = await roleService.list({ page, page_size: 10, search: debouncedSearch });
        if (active) {
          if (page > 1 && data.total_pages < page) {
            setPage(Math.max(1, data.total_pages));
            return;
          }
          setRolesData(data);
          setError(null);
        }
      } catch (err) {
        if (active) setError(extractApiError(err, 'Failed to load roles'));
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [page, debouncedSearch, reloadTick]);

  useEffect(() => {
    if (!hasPermission('permissions.read')) return;
    let active = true;
    fetchAllPages((catalogPage) => permissionService.list({ page: catalogPage, page_size: 100 }))
      .then((items) => {
        if (active) setAllPermissions(items);
      })
      .catch((err) => {
        if (active) setError(extractApiError(err, 'Failed to load permissions catalog'));
      });
    return () => {
      active = false;
    };
  }, [hasPermission]);

  const confirmDelete = (role) => {
    modals.openConfirmModal({
      title: 'Delete role',
      centered: true,
      children: (
        <Text size="sm">
          Delete custom role <strong>{role.name}</strong>? Any users currently assigned this role
          will have it removed immediately.
        </Text>
      ),
      labels: { confirm: 'Delete role', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: async () => {
        try {
          await roleService.remove(role.id);
          notifications.show({
            title: 'Role deleted',
            message: `Role "${role.name}" has been deleted.`,
            color: 'green',
          });
          triggerReload();
        } catch (err) {
          setError(extractApiError(err, 'Failed to delete role'));
        }
      },
    });
  };

  return (
    <Stack gap="md">
      <PageHeader
        title="Role Management"
        description="Configure RBAC roles and assign granular permissions grouped by resource"
        actions={
          <PermissionGuard permission="roles.create">
            <Button
              leftSection={<IconPlus size={16} />}
              onClick={() => {
                setEditingRole(null);
                setFormOpen(true);
              }}
            >
              Create Role
            </Button>
          </PermissionGuard>
        }
      />

      <ErrorAlert error={error} onClose={() => setError(null)} />

      <Paper withBorder p="md" radius="md">
        <TextInput
          placeholder="Search roles..."
          leftSection={<IconSearch size={16} />}
          value={search}
          onChange={(e) => {
            setSearch(e.currentTarget.value);
            setPage(1);
          }}
          mb="md"
        />

        {loading ? (
          <Stack gap="xs">
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} height={42} />
            ))}
          </Stack>
        ) : (
          <Table.ScrollContainer minWidth={640}>
            <Table striped highlightOnHover verticalSpacing="sm">
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Role Name</Table.Th>
                  <Table.Th>Description</Table.Th>
                  <Table.Th>Type</Table.Th>
                  <Table.Th>Permissions</Table.Th>
                  <Table.Th ta="right">Actions</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {rolesData.items.map((role) => (
                  <Table.Tr key={role.id}>
                    <Table.Td fw={600}>{role.name}</Table.Td>
                    <Table.Td>
                      <Text size="sm" c="dimmed">
                        {role.description}
                      </Text>
                    </Table.Td>
                    <Table.Td>
                      <Badge color={role.is_system ? 'indigo' : 'teal'} variant="light" radius="sm">
                        {role.is_system ? 'System' : 'Custom'}
                      </Badge>
                    </Table.Td>
                    <Table.Td>
                      <Badge variant="outline" color="gray">
                        {(role.permission_ids || []).length} permissions
                      </Badge>
                    </Table.Td>
                    <Table.Td>
                      <Group gap={4} justify="flex-end" wrap="nowrap">
                        <Tooltip label="View role details & assigned users">
                          <ActionIcon
                            variant="subtle"
                            aria-label={`View ${role.name}`}
                            onClick={() => setDetailRoleId(role.id)}
                          >
                            <IconEye size={16} />
                          </ActionIcon>
                        </Tooltip>

                        <PermissionGuard permission="roles.update">
                          <Tooltip label="Edit role & permissions">
                            <ActionIcon
                              variant="subtle"
                              color="indigo"
                              aria-label={`Edit ${role.name}`}
                              onClick={() => {
                                setEditingRole(role);
                                setFormOpen(true);
                              }}
                            >
                              <IconEdit size={16} />
                            </ActionIcon>
                          </Tooltip>
                        </PermissionGuard>

                        <PermissionGuard permission="roles.delete">
                          {!role.is_system && (
                            <Tooltip label="Delete role">
                              <ActionIcon
                                variant="subtle"
                                color="red"
                                aria-label={`Delete ${role.name}`}
                                onClick={() => confirmDelete(role)}
                              >
                                <IconTrash size={16} />
                              </ActionIcon>
                            </Tooltip>
                          )}
                        </PermissionGuard>
                      </Group>
                    </Table.Td>
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </Table.ScrollContainer>
        )}

        <Group justify="space-between" mt="md">
          <Text size="sm" c="dimmed">
            Showing {rolesData.items.length} of {rolesData.total} roles
          </Text>
          <Pagination
            total={Math.max(rolesData.total_pages || 1, 1)}
            value={page}
            onChange={setPage}
          />
        </Group>
      </Paper>

      <RoleFormModal
        key={`${editingRole?.id || 'new'}-${formOpen}`}
        opened={formOpen}
        onClose={() => setFormOpen(false)}
        role={editingRole}
        allPermissions={allPermissions}
        onPartialSaved={triggerReload}
        onSaved={() => {
          setFormOpen(false);
          triggerReload();
        }}
      />

      <RoleDetailModal roleId={detailRoleId} onClose={() => setDetailRoleId(null)} />
    </Stack>
  );
};

export default RolesPage;
