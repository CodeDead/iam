import { useCallback, useEffect, useState } from 'react';
import {
  ActionIcon,
  Badge,
  Button,
  Code,
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
import { IconEdit, IconPlus, IconSearch, IconShieldLock, IconTrash } from '@tabler/icons-react';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { PageHeader } from '../../components/common/PageHeader';
import { PermissionGuard } from '../../components/common/PermissionGuard';
import { usePermissions } from '../../hooks/usePermissions';
import { permissionService } from '../../services/permissionService';
import { extractApiError } from '../../utils/errors';
import { PermissionFormModal } from './PermissionFormModal';
import { PermissionRolesModal } from './PermissionRolesModal';

export function PermissionsPage() {
  const { hasPermission } = usePermissions();
  const [data, setData] = useState({ items: [], total: 0, page: 1, total_pages: 1 });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [reloadTick, setReloadTick] = useState(0);

  const [search, setSearch] = useState('');
  const [debouncedSearch] = useDebouncedValue(search, 300);
  const [page, setPage] = useState(1);

  const [formOpen, setFormOpen] = useState(false);
  const [editingPerm, setEditingPerm] = useState(null);
  const [rolesModalPerm, setRolesModalPerm] = useState(null);

  const triggerReload = useCallback(() => setReloadTick((n) => n + 1), []);

  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const res = await permissionService.list({ page, page_size: 15, search: debouncedSearch });
        if (active) {
          if (page > 1 && res.total_pages < page) {
            setPage(Math.max(1, res.total_pages));
            return;
          }
          setData(res);
          setError(null);
        }
      } catch (err) {
        if (active) setError(extractApiError(err, 'Failed to load permissions'));
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [page, debouncedSearch, reloadTick]);

  const confirmDelete = (perm) => {
    modals.openConfirmModal({
      title: 'Delete permission',
      centered: true,
      children: (
        <Text size="sm">
          Delete custom permission <strong>{perm.name}</strong>? It will be removed from all roles.
        </Text>
      ),
      labels: { confirm: 'Delete permission', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: async () => {
        try {
          await permissionService.remove(perm.id);
          notifications.show({
            title: 'Permission deleted',
            message: `Permission "${perm.name}" has been deleted.`,
            color: 'green',
          });
          triggerReload();
        } catch (err) {
          setError(extractApiError(err, 'Failed to delete permission'));
        }
      },
    });
  };

  return (
    <Stack gap="md">
      <PageHeader
        title="Permission Management"
        description="Manage granular resource.action permissions and inspect role associations"
        actions={
          <PermissionGuard permission="permissions.create">
            <Button
              leftSection={<IconPlus size={16} />}
              onClick={() => {
                setEditingPerm(null);
                setFormOpen(true);
              }}
            >
              Create Permission
            </Button>
          </PermissionGuard>
        }
      />

      <ErrorAlert error={error} onClose={() => setError(null)} />

      <Paper withBorder p="md" radius="md">
        <TextInput
          placeholder="Search permissions by name, resource, or description..."
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
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} height={40} />
            ))}
          </Stack>
        ) : (
          <Table.ScrollContainer minWidth={680}>
            <Table striped highlightOnHover verticalSpacing="sm">
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Name</Table.Th>
                  <Table.Th>Resource</Table.Th>
                  <Table.Th>Action</Table.Th>
                  <Table.Th>Description</Table.Th>
                  <Table.Th>Origin</Table.Th>
                  <Table.Th ta="right">Actions</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {data.items.map((perm) => (
                  <Table.Tr key={perm.id}>
                    <Table.Td>
                      <Code>{perm.name}</Code>
                    </Table.Td>
                    <Table.Td>{perm.resource}</Table.Td>
                    <Table.Td>{perm.action}</Table.Td>
                    <Table.Td>
                      <Text size="sm" c="dimmed">
                        {perm.description}
                      </Text>
                    </Table.Td>
                    <Table.Td>
                      <Badge
                        color={perm.is_system ? 'indigo' : 'teal'}
                        variant={perm.is_system ? 'light' : 'outline'}
                        radius="sm"
                      >
                        {perm.is_system ? 'System' : 'Custom'}
                      </Badge>
                    </Table.Td>
                    <Table.Td>
                      <Group gap={4} justify="flex-end" wrap="nowrap">
                        {hasPermission('roles.read') && (
                          <Tooltip label="View associated roles">
                            <ActionIcon
                              variant="subtle"
                              aria-label={`Roles for ${perm.name}`}
                              onClick={() => setRolesModalPerm(perm)}
                            >
                              <IconShieldLock size={16} />
                            </ActionIcon>
                          </Tooltip>
                        )}

                        <PermissionGuard permission="permissions.update">
                          {!perm.is_system && (
                            <Tooltip label="Edit permission">
                              <ActionIcon
                                variant="subtle"
                                color="indigo"
                                aria-label={`Edit ${perm.name}`}
                                onClick={() => {
                                  setEditingPerm(perm);
                                  setFormOpen(true);
                                }}
                              >
                                <IconEdit size={16} />
                              </ActionIcon>
                            </Tooltip>
                          )}
                        </PermissionGuard>

                        <PermissionGuard permission="permissions.delete">
                          {!perm.is_system && (
                            <Tooltip label="Delete permission">
                              <ActionIcon
                                variant="subtle"
                                color="red"
                                aria-label={`Delete ${perm.name}`}
                                onClick={() => confirmDelete(perm)}
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
            Showing {data.items.length} of {data.total} permissions
          </Text>
          <Pagination total={Math.max(data.total_pages || 1, 1)} value={page} onChange={setPage} />
        </Group>
      </Paper>

      <PermissionFormModal
        key={`${editingPerm?.id || 'new'}-${formOpen}`}
        opened={formOpen}
        onClose={() => setFormOpen(false)}
        permission={editingPerm}
        onSaved={() => {
          setFormOpen(false);
          triggerReload();
        }}
      />

      <PermissionRolesModal permission={rolesModalPerm} onClose={() => setRolesModalPerm(null)} />
    </Stack>
  );
}
