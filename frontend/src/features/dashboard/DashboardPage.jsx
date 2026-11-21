import { useEffect, useState } from 'react';
import {
  Badge,
  Card,
  Group,
  Paper,
  SimpleGrid,
  Skeleton,
  Stack,
  Text,
  ThemeIcon,
  Title,
} from '@mantine/core';
import {
  IconKey,
  IconShieldLock,
  IconUserCheck,
  IconUserOff,
  IconUsers,
} from '@tabler/icons-react';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { PageHeader } from '../../components/common/PageHeader';
import { PermissionGuard } from '../../components/common/PermissionGuard';
import { useAuth } from '../../hooks/useAuth';
import { dashboardService } from '../../services/dashboardService';
import { extractApiError } from '../../utils/errors';
import { formatUserName } from '../../utils/formatters';

function StatCard({ label, value, icon: Icon, color }) {
  return (
    <Paper withBorder p="md" radius="md">
      <Group justify="space-between" align="flex-start">
        <Stack gap={4}>
          <Text size="xs" tt="uppercase" fw={600} c="dimmed">
            {label}
          </Text>
          <Title order={2}>{value}</Title>
        </Stack>
        <ThemeIcon size="lg" radius="md" color={color} variant="light">
          <Icon size={20} />
        </ThemeIcon>
      </Group>
    </Paper>
  );
}

export function DashboardPage() {
  const { user, permissions } = useAuth();
  const [stats, setStats] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const data = await dashboardService.getStats();
        if (active) setStats(data);
      } catch (err) {
        if (active) setError(extractApiError(err, 'Failed to load dashboard statistics'));
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
    };
  }, []);

  return (
    <Stack gap="lg">
      <PageHeader
        title={`Welcome, ${formatUserName(user) || user?.email}`}
        description="Overview of your access level and authorized system metrics"
      />

      <ErrorAlert error={error} onClose={() => setError(null)} />

      {loading ? (
        <SimpleGrid cols={{ base: 1, sm: 2, lg: 5 }} spacing="md">
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} height={96} radius="md" />
          ))}
        </SimpleGrid>
      ) : (
        <SimpleGrid cols={{ base: 1, sm: 2, lg: 5 }} spacing="md">
          <PermissionGuard permission="users.read">
            {stats?.users && (
              <>
                <StatCard
                  label="Total Users"
                  value={stats.users.total}
                  icon={IconUsers}
                  color="indigo"
                />
                <StatCard
                  label="Active Users"
                  value={stats.users.active}
                  icon={IconUserCheck}
                  color="green"
                />
                <StatCard
                  label="Disabled Users"
                  value={stats.users.disabled}
                  icon={IconUserOff}
                  color="red"
                />
              </>
            )}
          </PermissionGuard>

          <PermissionGuard permission="roles.read">
            {stats?.total_roles !== undefined && stats?.total_roles !== null && (
              <StatCard
                label="Total Roles"
                value={stats.total_roles}
                icon={IconShieldLock}
                color="violet"
              />
            )}
          </PermissionGuard>

          <PermissionGuard permission="permissions.read">
            {stats?.total_permissions !== undefined && stats?.total_permissions !== null && (
              <StatCard
                label="Total Permissions"
                value={stats.total_permissions}
                icon={IconKey}
                color="teal"
              />
            )}
          </PermissionGuard>
        </SimpleGrid>
      )}

      <Card withBorder radius="md" p="lg">
        <Stack gap="sm">
          <Title order={4}>Your Assigned Roles &amp; Effective Permissions</Title>
          <Group gap="xs">
            {(user?.roles || []).map((role) => (
              <Badge key={role.id} color="indigo" variant="filled">
                {role.name}
              </Badge>
            ))}
          </Group>
          <Text size="sm" c="dimmed" mt="xs">
            Effective permissions ({permissions.length}):
          </Text>
          <Group gap="xs">
            {permissions.map((perm) => (
              <Badge key={perm} variant="light" color="gray">
                {perm}
              </Badge>
            ))}
          </Group>
        </Stack>
      </Card>
    </Stack>
  );
}
