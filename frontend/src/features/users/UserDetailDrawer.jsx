import { useEffect, useState } from 'react';
import { Anchor, Badge, Divider, Drawer, Group, Skeleton, Stack, Text, Title } from '@mantine/core';
import { Link } from 'react-router-dom';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { StatusBadge } from '../../components/common/StatusBadge';
import { usePermissions } from '../../hooks/usePermissions';
import { userService } from '../../services/userService';
import { extractApiError } from '../../utils/errors';
import {
  capitalize,
  formatDateTime,
  formatUserName,
  getMemberStatusColor,
} from '../../utils/formatters';

export function UserDetailDrawer({ userId, onClose }) {
  const { hasPermission } = usePermissions();
  const canReadOrgs = hasPermission('organisations.read');
  const [state, setState] = useState({
    loadedId: null,
    user: null,
    perms: [],
    orgs: [],
    error: null,
  });

  useEffect(() => {
    if (!userId) return;
    let active = true;
    Promise.all([
      userService.getById(userId),
      userService.getEffectivePermissions(userId),
      canReadOrgs ? userService.getOrganisations(userId) : Promise.resolve([]),
    ])
      .then(([u, perms, orgs]) => {
        if (active) {
          setState({
            loadedId: userId,
            user: u,
            perms: perms || [],
            orgs: orgs || [],
            error: null,
          });
        }
      })
      .catch((err) => {
        if (active) {
          setState({
            loadedId: userId,
            user: null,
            perms: [],
            orgs: [],
            error: extractApiError(err, 'Failed to load user details'),
          });
        }
      });

    return () => {
      active = false;
    };
  }, [userId, canReadOrgs]);

  const loading = Boolean(userId) && state.loadedId !== userId;
  const user = state.loadedId === userId ? state.user : null;
  const effectivePerms = state.loadedId === userId ? state.perms : [];
  const organisations = state.loadedId === userId ? state.orgs : [];
  const error = state.loadedId === userId ? state.error : null;

  return (
    <Drawer
      opened={Boolean(userId)}
      onClose={onClose}
      title="User Details"
      position="right"
      size="md"
    >
      <ErrorAlert error={error} />
      {loading ? (
        <Stack gap="sm">
          <Skeleton height={28} />
          <Skeleton height={120} />
          <Skeleton height={120} />
        </Stack>
      ) : (
        user && (
          <Stack gap="md">
            <div>
              <Title order={4}>{formatUserName(user)}</Title>
              <Text size="sm" c="dimmed">
                {user.email}
              </Text>
            </div>

            <Group gap="sm">
              <StatusBadge status={user.status} />
              <Badge variant="outline" color={user.email_verified ? 'green' : 'gray'}>
                {user.email_verified ? 'Email Verified' : 'Unverified'}
              </Badge>
            </Group>

            <Divider />

            <Stack gap={6}>
              <Text size="xs" c="dimmed">
                Created: {formatDateTime(user.created_at)}
              </Text>
              <Text size="xs" c="dimmed">
                Updated: {formatDateTime(user.updated_at)}
              </Text>
              <Text size="xs" c="dimmed">
                Last Login: {formatDateTime(user.last_login_at)}
              </Text>
              {user.profile?.job_title && (
                <Text size="xs">Job Title: {user.profile.job_title}</Text>
              )}
              {user.profile?.department && (
                <Text size="xs">Department: {user.profile.department}</Text>
              )}
            </Stack>

            <Divider />

            <div>
              <Text size="sm" fw={600} mb="xs">
                Assigned Roles ({(user.roles || []).length})
              </Text>
              <Group gap="xs">
                {(user.roles || []).map((r) => (
                  <Badge key={r.id} color="indigo">
                    {r.name}
                  </Badge>
                ))}
              </Group>
            </div>

            <div>
              <Text size="sm" fw={600} mb="xs">
                Effective Permissions ({effectivePerms.length})
              </Text>
              <Group gap="xs">
                {effectivePerms.map((p) => (
                  <Badge key={p.id} variant="light" color="gray">
                    {p.name}
                  </Badge>
                ))}
              </Group>
            </div>

            {canReadOrgs && (
              <div>
                <Text size="sm" fw={600} mb="xs">
                  Organisations ({organisations.length})
                </Text>
                {organisations.length === 0 ? (
                  <Text size="xs" c="dimmed">
                    Not a member of any organisation.
                  </Text>
                ) : (
                  <Stack gap="xs">
                    {organisations.map((org) => (
                      <Group key={org.id} justify="space-between" gap="xs" wrap="nowrap">
                        <Anchor
                          component={Link}
                          to={`/organisations?search=${encodeURIComponent(org.name)}`}
                          onClick={onClose}
                          size="sm"
                          fw={500}
                          truncate
                        >
                          {org.name}
                        </Anchor>
                        <Group gap={6} wrap="nowrap">
                          <Badge size="xs" variant="light" color="indigo">
                            {capitalize(org.role || 'member')}
                          </Badge>
                          <Badge size="xs" variant="light" color={getMemberStatusColor(org.status)}>
                            {capitalize(org.status)}
                          </Badge>
                        </Group>
                      </Group>
                    ))}
                  </Stack>
                )}
              </div>
            )}
          </Stack>
        )
      )}
    </Drawer>
  );
}
