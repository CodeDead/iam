import { useCallback, useEffect, useState } from 'react';
import {
  ActionIcon,
  Badge,
  Button,
  Code,
  Group,
  Modal,
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
import { IconEye, IconRefresh, IconSearch } from '@tabler/icons-react';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { PageHeader } from '../../components/common/PageHeader';
import { auditService } from '../../services/auditService';
import { extractApiError } from '../../utils/errors';
import { formatDateTime } from '../../utils/formatters';

const RESOURCE_TYPE_OPTIONS = [
  { value: '', label: 'All resources' },
  { value: 'permission', label: 'Permissions' },
  { value: 'role', label: 'Roles' },
  { value: 'user', label: 'Users' },
  { value: 'profile', label: 'Profile' },
  { value: 'auth', label: 'Authentication' },
];

const ACTION_OPTIONS = [
  { value: '', label: 'All actions' },
  { value: 'permission.created', label: 'permission.created' },
  { value: 'permission.updated', label: 'permission.updated' },
  { value: 'permission.deleted', label: 'permission.deleted' },
  { value: 'role.created', label: 'role.created' },
  { value: 'role.updated', label: 'role.updated' },
  { value: 'role.permissions_updated', label: 'role.permissions_updated' },
  { value: 'role.deleted', label: 'role.deleted' },
  { value: 'user.created', label: 'user.created' },
  { value: 'user.updated', label: 'user.updated' },
  { value: 'user.status_updated', label: 'user.status_updated' },
  { value: 'user.roles_updated', label: 'user.roles_updated' },
  { value: 'user.deleted', label: 'user.deleted' },
  { value: 'profile.updated', label: 'profile.updated' },
  { value: 'profile.password_changed', label: 'profile.password_changed' },
  { value: 'profile.account_deleted', label: 'profile.account_deleted' },
  { value: 'auth.registered', label: 'auth.registered' },
  { value: 'auth.login', label: 'auth.login' },
  { value: 'auth.logout', label: 'auth.logout' },
  { value: 'auth.refresh_token_reused', label: 'auth.refresh_token_reused' },
];

function getActionBadgeColor(action = '') {
  if (action.endsWith('deleted') || action.includes('reused')) return 'red';
  if (action.endsWith('.created') || action.endsWith('.registered')) return 'green';
  if (action.endsWith('.login')) return 'teal';
  if (action.endsWith('.logout')) return 'gray';
  return 'indigo';
}

export function AuditLogsPage() {
  const [data, setData] = useState({ items: [], total: 0, page: 1, total_pages: 1 });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [reloadTick, setReloadTick] = useState(0);

  const [search, setSearch] = useState('');
  const [debouncedSearch] = useDebouncedValue(search, 300);
  const [resourceType, setResourceType] = useState('');
  const [actionFilter, setActionFilter] = useState('');
  const [page, setPage] = useState(1);

  const [selectedEntry, setSelectedEntry] = useState(null);

  const triggerReload = useCallback(() => setReloadTick((n) => n + 1), []);

  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const res = await auditService.list({
          page,
          page_size: 20,
          search: debouncedSearch,
          resource_type: resourceType,
          action: actionFilter,
        });
        if (active) {
          setData(res);
          setError(null);
        }
      } catch (err) {
        if (active) setError(extractApiError(err, 'Failed to load audit logs'));
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [page, debouncedSearch, resourceType, actionFilter, reloadTick]);

  return (
    <Stack gap="md">
      <PageHeader
        title="Audit Logs"
        description="Inspect security, user, role, and permission activity across CodeDead IAM"
        actions={
          <Button variant="default" leftSection={<IconRefresh size={16} />} onClick={triggerReload}>
            Refresh
          </Button>
        }
      />

      <ErrorAlert error={error} onClose={() => setError(null)} />

      <Paper withBorder p="md" radius="md">
        <Group mb="md" grow>
          <TextInput
            placeholder="Search by actor, resource, action, or summary..."
            leftSection={<IconSearch size={16} />}
            value={search}
            onChange={(e) => {
              setSearch(e.currentTarget.value);
              setPage(1);
            }}
          />
          <Select
            placeholder="Filter by resource"
            data={RESOURCE_TYPE_OPTIONS}
            value={resourceType}
            onChange={(val) => {
              setResourceType(val || '');
              setPage(1);
            }}
            clearable
          />
          <Select
            placeholder="Filter by action"
            data={ACTION_OPTIONS}
            value={actionFilter}
            onChange={(val) => {
              setActionFilter(val || '');
              setPage(1);
            }}
            searchable
            clearable
          />
        </Group>

        {loading ? (
          <Stack gap="xs">
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} height={42} />
            ))}
          </Stack>
        ) : (
          <Table.ScrollContainer minWidth={780}>
            <Table striped highlightOnHover verticalSpacing="sm">
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Timestamp</Table.Th>
                  <Table.Th>Action</Table.Th>
                  <Table.Th>Resource</Table.Th>
                  <Table.Th>Actor</Table.Th>
                  <Table.Th>Summary</Table.Th>
                  <Table.Th ta="right">Details</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {data.items.length === 0 ? (
                  <Table.Tr>
                    <Table.Td colSpan={6}>
                      <Text ta="center" c="dimmed" py="md">
                        No audit log entries found matching your criteria.
                      </Text>
                    </Table.Td>
                  </Table.Tr>
                ) : (
                  data.items.map((entry) => (
                    <Table.Tr key={entry.id}>
                      <Table.Td>
                        <Text size="xs" c="dimmed" style={{ whiteSpace: 'nowrap' }}>
                          {formatDateTime(entry.created_at)}
                        </Text>
                      </Table.Td>
                      <Table.Td>
                        <Badge
                          size="sm"
                          variant="light"
                          color={getActionBadgeColor(entry.action)}
                          radius="sm"
                        >
                          {entry.action}
                        </Badge>
                      </Table.Td>
                      <Table.Td>
                        <Stack gap={2}>
                          <Text size="xs" tt="uppercase" fw={600} c="dimmed">
                            {entry.resource_type}
                          </Text>
                          {entry.resource_name && <Code>{entry.resource_name}</Code>}
                        </Stack>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm">{entry.actor_email || 'System'}</Text>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm">{entry.summary}</Text>
                      </Table.Td>
                      <Table.Td>
                        <Group justify="flex-end">
                          <Tooltip label="Inspect event details">
                            <ActionIcon
                              variant="subtle"
                              aria-label={`Inspect audit entry ${entry.action}`}
                              onClick={() => setSelectedEntry(entry)}
                            >
                              <IconEye size={16} />
                            </ActionIcon>
                          </Tooltip>
                        </Group>
                      </Table.Td>
                    </Table.Tr>
                  ))
                )}
              </Table.Tbody>
            </Table>
          </Table.ScrollContainer>
        )}

        <Group justify="space-between" mt="md">
          <Text size="sm" c="dimmed">
            Showing {data.items.length} of {data.total} audit events
          </Text>
          <Pagination total={Math.max(data.total_pages || 1, 1)} value={page} onChange={setPage} />
        </Group>
      </Paper>

      <Modal
        opened={Boolean(selectedEntry)}
        onClose={() => setSelectedEntry(null)}
        title="Audit Event Details"
        size="lg"
      >
        {selectedEntry && (
          <Stack gap="sm">
            <Group justify="space-between">
              <Badge
                size="md"
                variant="light"
                color={getActionBadgeColor(selectedEntry.action)}
                radius="sm"
              >
                {selectedEntry.action}
              </Badge>
              <Text size="xs" c="dimmed">
                {formatDateTime(selectedEntry.created_at)}
              </Text>
            </Group>

            <Text size="sm" fw={500}>
              {selectedEntry.summary}
            </Text>

            <Stack gap={4}>
              <Text size="xs" c="dimmed">
                Actor: {selectedEntry.actor_email || 'System'}
                {selectedEntry.actor_id ? ` (${selectedEntry.actor_id})` : ''}
              </Text>
              <Text size="xs" c="dimmed">
                Resource: {selectedEntry.resource_type}
                {selectedEntry.resource_name ? ` — ${selectedEntry.resource_name}` : ''}
                {selectedEntry.resource_id ? ` (${selectedEntry.resource_id})` : ''}
              </Text>
              <Text size="xs" c="dimmed">
                Event ID: {selectedEntry.id}
              </Text>
            </Stack>

            {selectedEntry.details && (
              <div>
                <Text size="xs" fw={600} mb={4}>
                  Metadata / Changes
                </Text>
                <Code block>{JSON.stringify(selectedEntry.details, null, 2)}</Code>
              </div>
            )}
          </Stack>
        )}
      </Modal>
    </Stack>
  );
}
