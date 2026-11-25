import { useCallback, useEffect, useMemo, useState } from 'react';
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
  IconBuilding,
  IconCheck,
  IconDevices,
  IconFileAnalytics,
  IconKey,
  IconMail,
  IconRefresh,
  IconShieldExclamation,
  IconShieldLock,
  IconUserCheck,
  IconUserCog,
  IconUsers,
  IconX,
} from '@tabler/icons-react';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import { Link } from 'react-router-dom';
import ErrorAlert from '../../components/common/ErrorAlert';
import PageHeader from '../../components/common/PageHeader';
import PermissionGuard from '../../components/common/PermissionGuard';
import { useAuth } from '../../hooks/useAuth';
import { usePermissions } from '../../hooks/usePermissions';
import { dashboardService } from '../../services/dashboardService';
import { organisationService } from '../../services/organisationService';
import { extractApiError } from '../../utils/errors';
import {
  capitalize,
  formatDateTime,
  formatUserName,
  getMemberStatusColor,
} from '../../utils/formatters';
import ActivityTrendChart from './ActivityTrendChart';
import OrganisationDistributionCard from './OrganisationDistributionCard';
import ResourceBreakdownCard from './ResourceBreakdownCard';
import RoleDistributionCard from './RoleDistributionCard';
import UserStatusDonutChart from './UserStatusDonutChart';

function getKpiColSpan(index, total) {
  switch (total) {
    case 1:
      return 12;
    case 2:
      return 6;
    case 3:
      return 4;
    case 4:
      return 3;
    case 5:
      return index < 3 ? 4 : 6;
    case 6:
      return 4;
    case 7:
      return index < 4 ? 3 : 4;
    default:
      return 3;
  }
}

const StatCard = ({ label, value, subtitle, icon: Icon, color }) => {
  return (
    <Paper withBorder p="md" radius="md" h="100%">
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
};

function getAuditBadgeColor(action = '') {
  if (action.endsWith('deleted') || action.includes('reused') || action.endsWith('_failed')) {
    return 'red';
  }
  if (
    action.endsWith('_cancelled') ||
    action.endsWith('_declined') ||
    action.endsWith('_removed')
  ) {
    return 'orange';
  }
  if (action.endsWith('.created') || action.endsWith('.registered')) return 'green';
  if (action.endsWith('.login')) return 'teal';
  if (action.endsWith('.logout')) return 'gray';
  return 'indigo';
}

const DISMISSED_REUSE_ALERT_PREFIX = 'codedead_iam_dismissed_token_reuse_alert';

function getDismissedReuseStorageKey(userId) {
  return userId ? `${DISMISSED_REUSE_ALERT_PREFIX}:${userId}` : DISMISSED_REUSE_ALERT_PREFIX;
}

function readDismissedReuseKey(userId) {
  try {
    return window.localStorage.getItem(getDismissedReuseStorageKey(userId));
  } catch {
    return null;
  }
}

function saveDismissedReuseKey(userId, value) {
  try {
    window.localStorage.setItem(getDismissedReuseStorageKey(userId), value);
  } catch {
    // Ignore storage quota or privacy-mode errors.
  }
}

const DashboardPage = () => {
  const { user, permissions } = useAuth();
  const { hasPermission, hasAnyPermission } = usePermissions();
  const canViewAccessSummary = hasAnyPermission(['roles.read', 'permissions.read']);
  const hasGlobalStats = hasAnyPermission([
    'users.read',
    'organisations.read',
    'roles.read',
    'permissions.read',
    'audit_logs.read',
  ]);
  const [stats, setStats] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [reloadTick, setReloadTick] = useState(0);
  const [busyInvites, setBusyInvites] = useState({});
  const [dismissedByUser, setDismissedByUser] = useState({});
  const storedDismissedKey = useMemo(() => readDismissedReuseKey(user?.id), [user?.id]);
  const activeDismissedKey = dismissedByUser[user?.id || ''] ?? storedDismissedKey;

  const setInviteBusy = useCallback((orgId, action) => {
    setBusyInvites((prev) => {
      if (!action) {
        const next = { ...prev };
        delete next[orgId];
        return next;
      }
      return { ...prev, [orgId]: action };
    });
  }, []);

  const handleRefresh = useCallback(() => {
    setLoading(true);
    setReloadTick((n) => n + 1);
  }, []);

  const handleAcceptInvite = useCallback(
    async (org) => {
      setInviteBusy(org.id, 'accept');
      try {
        await organisationService.acceptInvitation(org.id);
        notifications.show({
          title: 'Invitation accepted',
          message: `You are now an active member of '${org.name}'.`,
          color: 'green',
        });
        setReloadTick((n) => n + 1);
      } catch (err) {
        setError(extractApiError(err, 'Failed to accept invitation'));
      } finally {
        setInviteBusy(org.id, null);
      }
    },
    [setInviteBusy]
  );

  const handleDeclineInvite = useCallback(
    (org) => {
      modals.openConfirmModal({
        title: 'Decline invitation',
        centered: true,
        children: (
          <Text size="sm">
            Are you sure you want to decline the invitation to <strong>{org.name}</strong>?
          </Text>
        ),
        labels: { confirm: 'Decline', cancel: 'Cancel' },
        confirmProps: { color: 'red' },
        onConfirm: async () => {
          setInviteBusy(org.id, 'decline');
          try {
            await organisationService.leave(org.id);
            notifications.show({
              title: 'Invitation declined',
              message: `Declined invitation to '${org.name}'.`,
              color: 'green',
            });
            setReloadTick((n) => n + 1);
          } catch (err) {
            setError(extractApiError(err, 'Failed to decline invitation'));
          } finally {
            setInviteBusy(org.id, null);
          }
        },
      });
    },
    [setInviteBusy]
  );

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

  const tokenReuseCount24h = stats?.audit?.token_reuse_alerts_last_24h || 0;
  const tokenReuseAlertKey =
    tokenReuseCount24h > 0
      ? `${tokenReuseCount24h}:${stats?.audit?.latest_token_reuse_at || ''}`
      : null;
  const showTokenReuseBanner =
    !loading && Boolean(tokenReuseAlertKey) && activeDismissedKey !== tokenReuseAlertKey;

  const handleDismissTokenReuseAlert = useCallback(() => {
    if (!tokenReuseAlertKey) return;
    setDismissedByUser((prev) => ({ ...prev, [user?.id || '']: tokenReuseAlertKey }));
    saveDismissedReuseKey(user?.id, tokenReuseAlertKey);
  }, [tokenReuseAlertKey, user?.id]);

  const showActivityChart = Boolean(hasPermission('audit_logs.read') && stats?.audit);
  const showUserStatusChart = Boolean(hasPermission('users.read') && stats?.users);

  const globalKpiCards = [];
  if (hasPermission('users.read') && stats?.users) {
    globalKpiCards.push(
      {
        key: 'total-users',
        label: 'Total Users',
        value: stats.users.total,
        subtitle: `+${stats.users.new_last_7_days} in last 7 days`,
        icon: IconUsers,
        color: 'indigo',
      },
      {
        key: 'active-users',
        label: 'Active Users',
        value: stats.users.active,
        subtitle: `${activeUserPct}% active · ${stats.users.disabled + stats.users.locked} restricted`,
        icon: IconUserCheck,
        color: 'green',
      },
      {
        key: 'active-sessions',
        label: 'Active Sessions',
        value: stats.users.active_sessions,
        subtitle: 'Valid refresh tokens across system',
        icon: IconDevices,
        color: 'cyan',
      }
    );
  }
  if (hasPermission('organisations.read') && stats?.organisations) {
    globalKpiCards.push({
      key: 'organisations',
      label: 'Organisations',
      value: stats.organisations.total,
      subtitle: `+${stats.organisations.new_last_7_days} in 7d · ${stats.organisations.active_memberships} active members`,
      icon: IconBuilding,
      color: 'grape',
    });
  }
  if (hasPermission('roles.read') && stats?.roles) {
    globalKpiCards.push({
      key: 'roles',
      label: 'Total Roles',
      value: stats.roles.total,
      subtitle: `${stats.roles.system} system · ${stats.roles.custom} custom`,
      icon: IconShieldLock,
      color: 'violet',
    });
  }
  if (hasPermission('permissions.read') && stats?.permissions) {
    globalKpiCards.push({
      key: 'permissions',
      label: 'Total Permissions',
      value: stats.permissions.total,
      subtitle: `${stats.permissions.system} system · ${stats.permissions.custom} custom`,
      icon: IconKey,
      color: 'teal',
    });
  }
  if (hasPermission('audit_logs.read') && stats?.audit) {
    globalKpiCards.push(
      {
        key: 'audit-24h',
        label: 'Audit Events (24h)',
        value: stats.audit.events_last_24h,
        subtitle: `${stats.audit.events_last_7d} in 7d · ${stats.audit.total} total`,
        icon: IconActivity,
        color: 'blue',
      },
      {
        key: 'failed-logins-24h',
        label: 'Failed Logins (24h)',
        value: stats.audit.failed_logins_last_24h,
        subtitle:
          stats.audit.failed_logins_last_24h > 0
            ? 'Review auth.login_failed in Audit Logs'
            : 'No failed sign-ins in last 24h',
        icon: IconAlertTriangle,
        color: stats.audit.failed_logins_last_24h > 0 ? 'red' : 'teal',
      }
    );
  }

  const myOrgsCount = stats?.my_organisations?.length || 0;
  const activeMyOrgsCount = (stats?.my_organisations || []).filter(
    (o) => String(o.status || '').toLowerCase() === 'active'
  ).length;

  const kpiCards =
    globalKpiCards.length > 0
      ? globalKpiCards
      : stats
        ? [
            {
              key: 'my-sessions',
              label: 'Your Active Sessions',
              value: stats.my_active_sessions,
              subtitle: `Last login: ${formatDateTime(user?.last_login_at)}`,
              icon: IconDevices,
              color: 'indigo',
            },
            {
              key: 'my-organisations',
              label: 'Your Organisations',
              value: myOrgsCount,
              subtitle:
                myOrgsCount > 0
                  ? `${activeMyOrgsCount} active ${activeMyOrgsCount === 1 ? 'membership' : 'memberships'}`
                  : 'Personal account',
              icon: IconBuilding,
              color: 'grape',
            },
          ]
        : [];

  const distributionCards = [];
  if (hasPermission('organisations.read') && stats?.organisations) {
    distributionCards.push(
      <OrganisationDistributionCard key="organisations" organisations={stats.organisations} />
    );
  }
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

  // Keep rows visually balanced with zero orphaned half-width cards: 1->1, 2->2, 3->3, 4->2 (2x2)
  const distributionColsLg =
    distributionCards.length === 4 ? 2 : Math.max(1, distributionCards.length);

  const pendingOrgInvites = (stats?.my_organisations || []).filter(
    (o) => String(o.status || '').toLowerCase() === 'invited'
  );

  return (
    <Stack gap="lg">
      <PageHeader
        title={`Welcome, ${formatUserName(user) || user?.email}`}
        description={
          canViewAccessSummary || hasGlobalStats
            ? 'Real-time overview of your identity security posture, access control, and activity'
            : 'Overview of your active sessions, organisations, and quick navigation'
        }
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

      {!loading && pendingOrgInvites.length > 0 && (
        <Alert
          color="indigo"
          variant="light"
          radius="md"
          icon={<IconMail size={20} />}
          title={`Pending Organisation ${pendingOrgInvites.length === 1 ? 'Invitation' : 'Invitations'}`}
        >
          <Stack gap="xs" mt="xs">
            {pendingOrgInvites.map((invite) => (
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
                    loading={busyInvites[invite.id] === 'accept'}
                    disabled={Boolean(busyInvites[invite.id])}
                    onClick={() => handleAcceptInvite(invite)}
                  >
                    Accept
                  </Button>
                  <Button
                    size="xs"
                    variant="default"
                    color="red"
                    leftSection={<IconX size={14} />}
                    loading={busyInvites[invite.id] === 'decline'}
                    disabled={Boolean(busyInvites[invite.id])}
                    onClick={() => handleDeclineInvite(invite)}
                  >
                    Decline
                  </Button>
                </Group>
              </Group>
            ))}
          </Stack>
        </Alert>
      )}

      <PermissionGuard permission="audit_logs.read">
        {showTokenReuseBanner && (
          <Alert
            color="red"
            variant="light"
            radius="md"
            withCloseButton
            closeButtonLabel="Dismiss token reuse alert"
            onClose={handleDismissTokenReuseAlert}
            icon={<IconShieldExclamation size={20} />}
            title="Refresh Token Reuse Detected"
          >
            <Group justify="space-between" align="center" wrap="wrap" gap="sm">
              <Text size="sm">
                {tokenReuseCount24h} refresh token reuse{' '}
                {tokenReuseCount24h === 1 ? 'alert was' : 'alerts were'} triggered in the last 24
                hours
                {stats?.audit?.latest_token_reuse_at
                  ? ` (latest: ${formatDateTime(stats.audit.latest_token_reuse_at)})`
                  : ''}
                . Affected token families were automatically revoked.
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

      {loading && !stats ? (
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
          {kpiCards.length > 0 && (
            <Grid gutter="md" align="stretch">
              {kpiCards.map((card, index) => (
                <Grid.Col
                  key={card.key}
                  span={{ base: 12, sm: 6, lg: getKpiColSpan(index, kpiCards.length) }}
                >
                  <StatCard
                    label={card.label}
                    value={card.value}
                    subtitle={card.subtitle}
                    icon={card.icon}
                    color={card.color}
                  />
                </Grid.Col>
              ))}
            </Grid>
          )}

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
                lg: distributionColsLg,
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
              <Title order={4}>
                {canViewAccessSummary ? 'Your Access & Quick Navigation' : 'Quick Navigation'}
              </Title>
              <Text size="xs" c="dimmed">
                {canViewAccessSummary
                  ? 'Assigned roles, effective permissions, and authorized areas'
                  : 'Quick access to your available areas'}
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
              <Button
                component={Link}
                to="/organisations"
                size="xs"
                variant="default"
                leftSection={<IconBuilding size={14} />}
              >
                Organisations
              </Button>
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
              <PermissionGuard permission="profile.read">
                <Button
                  component={Link}
                  to="/profile"
                  size="xs"
                  variant="light"
                  leftSection={<IconUserCog size={14} />}
                >
                  My Profile
                </Button>
              </PermissionGuard>
            </Group>
          </Group>

          {(canViewAccessSummary || globalKpiCards.length > 0) && (
            <>
              <Group gap="xs" mt="xs">
                {canViewAccessSummary &&
                  (user?.roles || []).map((role) => (
                    <Badge key={role.id} color="indigo" variant="filled">
                      {role.name}
                    </Badge>
                  ))}
                {stats && globalKpiCards.length > 0 && (
                  <Badge variant="light" color="cyan" leftSection={<IconDevices size={12} />}>
                    Your active sessions: {stats.my_active_sessions}
                  </Badge>
                )}
                {user?.last_login_at && (
                  <Badge variant="light" color="gray">
                    Last sign-in: {formatDateTime(user.last_login_at)}
                  </Badge>
                )}
              </Group>

              {stats?.my_organisations?.length > 0 && (
                <>
                  <Text size="sm" c="dimmed" mt="xs">
                    Your organisations ({stats.my_organisations.length}):
                  </Text>
                  <Group gap="xs">
                    {stats.my_organisations.map((org) => (
                      <Badge
                        key={org.id}
                        variant="light"
                        color={getMemberStatusColor(org.status)}
                        leftSection={<IconBuilding size={12} />}
                      >
                        {org.name} ({capitalize(org.role || 'member')} · {capitalize(org.status)} ·{' '}
                        {org.member_count} {org.member_count === 1 ? 'member' : 'members'})
                      </Badge>
                    ))}
                  </Group>
                </>
              )}

              {canViewAccessSummary && (
                <>
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
                </>
              )}
            </>
          )}
        </Stack>
      </Card>
    </Stack>
  );
};

export default DashboardPage;
