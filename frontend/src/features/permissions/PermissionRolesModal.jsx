import { useEffect, useState } from 'react';
import { Badge, Code, Group, Modal, Skeleton, Stack, Text } from '@mantine/core';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { permissionService } from '../../services/permissionService';
import { extractApiError } from '../../utils/errors';

export function PermissionRolesModal({ permission, onClose }) {
  const [state, setState] = useState({ loadedId: null, roles: [], error: null });
  const permissionId = permission?.id || null;

  useEffect(() => {
    if (!permissionId) return;
    let active = true;
    permissionService
      .getAssociatedRoles(permissionId)
      .then((res) => {
        if (active) setState({ loadedId: permissionId, roles: res || [], error: null });
      })
      .catch((err) => {
        if (active) {
          setState({
            loadedId: permissionId,
            roles: [],
            error: extractApiError(err, 'Failed to load associated roles'),
          });
        }
      });

    return () => {
      active = false;
    };
  }, [permissionId]);

  const loading = Boolean(permissionId) && state.loadedId !== permissionId;
  const roles = state.loadedId === permissionId ? state.roles : [];
  const error = state.loadedId === permissionId ? state.error : null;

  return (
    <Modal
      opened={Boolean(permission)}
      onClose={onClose}
      title="Roles Associated with Permission"
      size="md"
    >
      <Stack gap="md">
        {permission && (
          <Group gap="xs">
            <Code>{permission.name}</Code>
            <Text size="sm" c="dimmed">
              — {permission.description}
            </Text>
          </Group>
        )}

        <ErrorAlert error={error} />

        {loading ? (
          <Skeleton height={80} />
        ) : roles.length === 0 ? (
          <Text size="sm" c="dimmed">
            No roles currently include this permission.
          </Text>
        ) : (
          <Stack gap="xs">
            {roles.map((role) => (
              <Group key={role.id} justify="space-between">
                <div>
                  <Text size="sm" fw={600}>
                    {role.name}
                  </Text>
                  <Text size="xs" c="dimmed">
                    {role.description}
                  </Text>
                </div>
                <Badge size="sm" color={role.is_system ? 'indigo' : 'teal'} variant="light">
                  {role.is_system ? 'System' : 'Custom'}
                </Badge>
              </Group>
            ))}
          </Stack>
        )}
      </Stack>
    </Modal>
  );
}
