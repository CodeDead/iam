import { useCallback, useEffect, useState } from 'react';
import {
  Alert,
  Badge,
  Button,
  Card,
  Grid,
  Group,
  Paper,
  SimpleGrid,
  Skeleton,
  Stack,
  Table,
  Text,
  ThemeIcon,
  Title,
} from '@mantine/core';
import {
  IconActivity,
  IconAlertTriangle,
  IconArrowRight,
  IconDevices,
  IconFileAnalytics,
  IconKey,
  IconRefresh,
  IconShieldExclamation,
  IconShieldLock,
  IconUserCheck,
  IconUserCog,
  IconUsers,
} from '@tabler/icons-react';
import { Link } from 'react-router-dom';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { PageHeader } from '../../components/common/PageHeader';
import { PermissionGuard } from '../../components/common/PermissionGuard';
import { useAuth } from '../../hooks/useAuth';
import { usePermissions } from '../../hooks/usePermissions';
import { dashboardService } from '../../services/dashboardService';
import { extractApiError } from '../../utils/errors';
import { formatDateTime, formatUserName } from '../../utils/formatters';
import {
  ActivityTrendChart,
  ResourceBreakdownCard,
  RoleDistributionCard,
  UserStatusDonutChart,
} from './DashboardCharts';

function StatCard({ label, value, subtitle, icon: Icon, color }) {
  return (
    <Paper withBorder p="md" radius="md">
      <Group justify="space-between" align="flex-start" wrap="nowrap">
        <Stack gap={2}>
          <Text size="xs" tt="uppercase" fw={600} c="dimmed">
            {label}
          </Text>
          <Title order={2}>{value}</Title>
          {subtitle && (
            <Text size="xs" c="dimmed">
              {subtitle}
            </Text>
          )}
        </Stack>
        <ThemeIcon size="lg" radius="md" color={color} variant="light">
          <Icon size={20} />
        </ThemeIcon>
      </Group>
    </Paper>
  );
}

function getAuditBadgeColor(action = '') {
  if (action.endsWith('deleted') || action.includes('reused') || action.endsWith('_failed')) {
    return 'red';
  }
  if (action.endsWith('.created') || action.endsWith('.registered')) return 'green';
  if (action.endsWith('.login')) return 'teal';
  if (action.endsWith('.logout')) return 'gray';
  return 'indigo';
}

export function DashboardPage() {
  const { user, permissions } = useAuth();
  const { hasPermission } = usePermissions();
  const [stats, setStats] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [reloadTick, setReloadTick] = useState(0);

  const handleRefresh = useCallback(() => {
    setLoading(true);
    setReloadTick((n) => n + 1);
  }, []);

  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const data = await dashboardService.getStats();
        if (active) {
          setStats(data);
          setError(null);
        }
      } catch (err) {
        if (active) setError(extractApiError(err, 'Failed to load dashboard statistics'));
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [reloadTick]);

  const activeUserPct =
    stats?.users && stats.users.total > 0
      ? Math.round((stats.users.active / stats.users.total) * 100)
      : 0;

  const showActivityChart = Boolean(hasPermission('audit_logs.read') && stats?.audit);
  const showUserStatusChart = Boolean(hasPermission('users.read') && stats?.users);

  const distributionCards = [];
  if (hasPermission('roles.read') && stats?.roles) {
    distributionCards.push(<RoleDistributionCard key="roles" roles={stats.roles} />);
  }
  if (hasPermission('permissions.read') && stats?.permissions) {
    distributionCards.push(
      <ResourceBreakdownCard
        key="permissions"
        title="Permissions by Resource"
        subtitle="Granular RBAC permissions grouped by domain"
        items={stats.permissions.by_resource}
        badgeColor="teal"
      />
    );
  }
  if (hasPermission('audit_logs.read') && stats?.audit) {
    distributionCards.push(
      <ResourceBreakdownCard
        key="audit-resources"
        title="Audit Events by Resource (7d)"
        subtitle="Security and administration activity by category"
        items={stats.audit.by_resource_type}
        badgeColor="blue"
      />
    );
  }

  return (
    <Stack gap="lg">
      <PageHeader
        title={`Welcome, ${formatUserName(user) || user?.email}`}
        description="Real-time overview of your identity security posture, access control, and activity"
        actions={
          <Button
            variant="default"
            leftSection={<IconRefresh size={16} />}
            onClick={handleRefresh}
            loading={loading}
          >
            Refresh
          </Button>
        }
      />

      <ErrorAlert error={error} onClose={() => setError(null)} />

      <PermissionGuard permission="audit_logs.read">
        {!loading && stats?.audit?.token_reuse_alerts_last_7d > 0 && (
          <Alert
            color="red"
            variant="light"
            radius="md"
            icon={<IconShieldExclamation size={20} />}
            title="Refresh Token Reuse Detected"
          >
            <Group justify="space-between" align="center" wrap="wrap" gap="sm">
              <Text size="sm">
                {stats.audit.token_reuse_alerts_last_7d} refresh token reuse{' '}
                {stats.audit.token_reuse_alerts_last_7d === 1 ? 'alert was' : 'alerts were'}{' '}
                triggered in the last 7 days. Affected token families were automatically revoked.
              </Text>
              <Button
                component={Link}
                to="/audit-logs"
                size="xs"
                color="red"
                variant="filled"
                rightSection={<IconArrowRight size={14} />}
              >
                Inspect Audit Logs
              </Button>
            </Group>
          </Alert>
        )}
      </PermissionGuard>

      {loading ? (
        <>
          <SimpleGrid cols={{ base: 1, sm: 2, lg: 4 }} spacing="md">
            {Array.from({ length: 4 }).map((_, i) => (
              <Skeleton key={i} height={106} radius="md" />
            ))}
          </SimpleGrid>
          <Grid gutter="md">
            <Grid.Col span={{ base: 12, lg: 7 }}>
              <Skeleton height={260} radius="md" />
            </Grid.Col>
            <Grid.Col span={{ base: 12, lg: 5 }}>
              <Skeleton height={260} radius="md" />
            </Grid.Col>
          </Grid>
          <SimpleGrid cols={{ base: 1, lg: 3 }} spacing="md">
            {Array.from({ length: 3 }).map((_, i) => (
              <Skeleton key={i} height={220} radius="md" />
            ))}
          </SimpleGrid>
        </>
      ) : (
        <>
          <SimpleGrid cols={{ base: 1, sm: 2, md: 3, xl: 4 }} spacing="md">
            <PermissionGuard permission="users.read">
              {stats?.users && (
                <>
                  <StatCard
                    label="Total Users"
                    value={stats.users.total}
                    subtitle={`+${stats.users.new_last_7_days} in last 7 days`}
                    icon={IconUsers}
                    color="indigo"
                  />
                  <StatCard
                    label="Active Users"
                    value={stats.users.active}
                    subtitle={`${activeUserPct}% active · ${stats.users.disabled + stats.users.locked} restricted`}
                    icon={IconUserCheck}
                    color="green"
                  />
                  <StatCard
                    label="Active Sessions"
                    value={stats.users.active_sessions}
                    subtitle="Valid refresh tokens across system"
                    icon={IconDevices}
                    color="cyan"
                  />
                </>
              )}
            </PermissionGuard>

            <PermissionGuard permission="roles.read">
              {stats?.roles && (
                <StatCard
                  label="Total Roles"
                  value={stats.roles.total}
                  subtitle={`${stats.roles.system} system · ${stats.roles.custom} custom`}
                  icon={IconShieldLock}
                  color="violet"
                />
              )}
            </PermissionGuard>

            <PermissionGuard permission="permissions.read">
              {stats?.permissions && (
                <StatCard
                  label="Total Permissions"
                  value={stats.permissions.total}
                  subtitle={`${stats.permissions.system} system · ${stats.permissions.custom} custom`}
                  icon={IconKey}
                  color="teal"
                />
              )}
            </PermissionGuard>

            <PermissionGuard permission="audit_logs.read">
              {stats?.audit && (
                <>
                  <StatCard
                    label="Audit Events (24h)"
                    value={stats.audit.events_last_24h}
                    subtitle={`${stats.audit.events_last_7d} in 7d · ${stats.audit.total} total`}
                    icon={IconActivity}
                    color="blue"
                  />
                  <StatCard
                    label="Failed Logins (24h)"
                    value={stats.audit.failed_logins_last_24h}
                    subtitle={
                      stats.audit.failed_logins_last_24h > 0
                        ? 'Review auth.login_failed in Audit Logs'
                        : 'No failed sign-ins in last 24h'
                    }
                    icon={IconAlertTriangle}
                    color={stats.audit.failed_logins_last_24h > 0 ? 'red' : 'teal'}
                  />
                </>
              )}
            </PermissionGuard>

            {stats && (
              <StatCard
                label="Your Active Sessions"
                value={stats.my_active_sessions}
                subtitle={`Last login: ${formatDateTime(user?.last_login_at)}`}
                icon={IconUserCog}
                color="indigo"
              />
            )}
          </SimpleGrid>

          {(showActivityChart || showUserStatusChart) && (
            <Grid gutter="md" align="stretch">
              {showActivityChart && (
                <Grid.Col span={{ base: 12, lg: showUserStatusChart ? 7 : 12 }}>
                  <ActivityTrendChart dailyActivity={stats.audit.daily_activity} />
                </Grid.Col>
              )}
              {showUserStatusChart && (
                <Grid.Col span={{ base: 12, lg: showActivityChart ? 5 : 12 }}>
                  <UserStatusDonutChart users={stats.users} />
                </Grid.Col>
              )}
            </Grid>
          )}

          {distributionCards.length > 0 && (
            <SimpleGrid
              cols={{
                base: 1,
                lg: distributionCards.length,
              }}
              spacing="md"
            >
              {distributionCards}
            </SimpleGrid>
          )}

          <PermissionGuard permission="audit_logs.read">
            {stats?.audit && (
              <Card withBorder radius="md" p="lg">
                <Stack gap="md">
                  <Group justify="space-between" align="center" wrap="wrap">
                    <div>
                      <Title order={4}>Recent Security &amp; Audit Activity</Title>
                      <Text size="xs" c="dimmed">
                        Latest recorded authentication and RBAC actions
                      </Text>
                    </div>
                    <Button
                      component={Link}
                      to="/audit-logs"
                      variant="light"
                      size="xs"
                      rightSection={<IconArrowRight size={14} />}
                    >
                      View all audit logs
                    </Button>
                  </Group>

                  {stats.audit.recent_events.length === 0 ? (
                    <Text size="sm" c="dimmed">
                      No audit events have been recorded yet.
                    </Text>
                  ) : (
                    <Table.ScrollContainer minWidth={640}>
                      <Table striped highlightOnHover verticalSpacing="xs">
                        <Table.Thead>
                          <Table.Tr>
                            <Table.Th>Time</Table.Th>
                            <Table.Th>Action</Table.Th>
                            <Table.Th>Actor</Table.Th>
                            <Table.Th>Summary</Table.Th>
                          </Table.Tr>
                        </Table.Thead>
                        <Table.Tbody>
                          {stats.audit.recent_events.map((event) => (
                            <Table.Tr key={event.id}>
                              <Table.Td>
                                <Text size="xs" c="dimmed" style={{ whiteSpace: 'nowrap' }}>
                                  {formatDateTime(event.created_at)}
                                </Text>
                              </Table.Td>
                              <Table.Td>
                                <Badge
                                  size="xs"
                                  variant="light"
                                  color={getAuditBadgeColor(event.action)}
                                >
                                  {event.action}
                                </Badge>
                              </Table.Td>
                              <Table.Td>
                                <Text size="xs">{event.actor_email || 'System'}</Text>
                              </Table.Td>
                              <Table.Td>
                                <Text size="sm">{event.summary}</Text>
                              </Table.Td>
                            </Table.Tr>
                          ))}
                        </Table.Tbody>
                      </Table>
                    </Table.ScrollContainer>
                  )}
                </Stack>
              </Card>
            )}
          </PermissionGuard>
        </>
      )}

      <Card withBorder radius="md" p="lg">
        <Stack gap="sm">
          <Group justify="space-between" align="center" wrap="wrap" gap="sm">
            <div>
              <Title order={4}>Your Access &amp; Quick Navigation</Title>
              <Text size="xs" c="dimmed">
                Assigned roles, effective permissions, and authorized areas
              </Text>
            </div>
            <Group gap="xs" wrap="wrap">
              <PermissionGuard permission="users.read">
                <Button
                  component={Link}
                  to="/users"
                  size="xs"
                  variant="default"
                  leftSection={<IconUsers size={14} />}
                >
                  Users
                </Button>
              </PermissionGuard>
              <PermissionGuard permission="roles.read">
                <Button
                  component={Link}
                  to="/roles"
                  size="xs"
                  variant="default"
                  leftSection={<IconShieldLock size={14} />}
                >
                  Roles
                </Button>
              </PermissionGuard>
              <PermissionGuard permission="permissions.read">
                <Button
                  component={Link}
                  to="/permissions"
                  size="xs"
                  variant="default"
                  leftSection={<IconKey size={14} />}
                >
                  Permissions
                </Button>
              </PermissionGuard>
              <PermissionGuard permission="audit_logs.read">
                <Button
                  component={Link}
                  to="/audit-logs"
                  size="xs"
                  variant="default"
                  leftSection={<IconFileAnalytics size={14} />}
                >
                  Audit Logs
                </Button>
              </PermissionGuard>
              <Button
                component={Link}
                to="/profile"
                size="xs"
                variant="light"
                leftSection={<IconUserCog size={14} />}
              >
                My Profile
              </Button>
            </Group>
          </Group>

          <Group gap="xs" mt="xs">
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
