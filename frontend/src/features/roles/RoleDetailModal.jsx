import { useEffect, useState } from 'react';
import { Badge, Divider, Group, Modal, Skeleton, Stack, Table, Text, Title } from '@mantine/core';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { StatusBadge } from '../../components/common/StatusBadge';
import { usePermissions } from '../../hooks/usePermissions';
import { roleService } from '../../services/roleService';
import { extractApiError } from '../../utils/errors';
import { formatUserName } from '../../utils/formatters';

export function RoleDetailModal({ roleId, onClose }) {
  const { hasPermission } = usePermissions();
  const [state, setState] = useState({ loadedId: null, role: null, users: [], error: null });

  const canReadUsers = hasPermission('users.read');

  useEffect(() => {
    if (!roleId) return;
    let active = true;

    const requests = [roleService.getById(roleId)];
    if (canReadUsers) {
      requests.push(roleService.getAssignedUsers(roleId));
    }

    Promise.all(requests)
      .then(([roleDetails, users]) => {
        if (active) {
          setState({ loadedId: roleId, role: roleDetails, users: users || [], error: null });
        }
      })
      .catch((err) => {
        if (active) {
          setState({
            loadedId: roleId,
            role: null,
            users: [],
            error: extractApiError(err, 'Failed to load role details'),
          });
        }
      });

    return () => {
      active = false;
    };
  }, [roleId, canReadUsers]);

  const loading = Boolean(roleId) && state.loadedId !== roleId;
  const role = state.loadedId === roleId ? state.role : null;
  const assignedUsers = state.loadedId === roleId ? state.users : [];
  const error = state.loadedId === roleId ? state.error : null;

  return (
    <Modal opened={Boolean(roleId)} onClose={onClose} title="Role Details" size="lg">
      <ErrorAlert error={error} />
      {loading ? (
        <Stack gap="sm">
          <Skeleton height={28} />
          <Skeleton height={100} />
        </Stack>
      ) : (
        role && (
          <Stack gap="md">
            <Group justify="space-between">
              <div>
                <Title order={4}>{role.name}</Title>
                <Text size="sm" c="dimmed">
                  {role.description}
                </Text>
              </div>
              <Badge color={role.is_system ? 'indigo' : 'teal'}>
                {role.is_system ? 'System Role' : 'Custom Role'}
              </Badge>
            </Group>

            <Divider
              label={`Permissions (${(role.permissions || []).length})`}
              labelPosition="left"
            />

            <Group gap="xs">
              {(role.permissions || []).map((perm) => (
                <Badge key={perm.id} variant="light" color="gray">
                  {perm.name}
                </Badge>
              ))}
            </Group>

            {canReadUsers && (
              <>
                <Divider label={`Assigned Users (${assignedUsers.length})`} labelPosition="left" />
                {assignedUsers.length === 0 ? (
                  <Text size="sm" c="dimmed">
                    No users are currently assigned this role.
                  </Text>
                ) : (
                  <Table striped verticalSpacing="xs">
                    <Table.Thead>
                      <Table.Tr>
                        <Table.Th>Name</Table.Th>
                        <Table.Th>Email</Table.Th>
                        <Table.Th>Status</Table.Th>
                      </Table.Tr>
                    </Table.Thead>
                    <Table.Tbody>
                      {assignedUsers.map((u) => (
                        <Table.Tr key={u.id}>
                          <Table.Td>{formatUserName(u)}</Table.Td>
                          <Table.Td>{u.email}</Table.Td>
                          <Table.Td>
                            <StatusBadge status={u.status} />
                          </Table.Td>
                        </Table.Tr>
                      ))}
                    </Table.Tbody>
                  </Table>
                )}
              </>
            )}
          </Stack>
        )
      )}
    </Modal>
  );
}
