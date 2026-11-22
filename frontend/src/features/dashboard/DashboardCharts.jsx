import {
  Badge,
  Box,
  Card,
  Center,
  Group,
  Progress,
  RingProgress,
  Stack,
  Text,
  Title,
  Tooltip,
} from '@mantine/core';
import { capitalize } from '../../utils/formatters';

function formatShortUtcDate(dateStr) {
  if (!dateStr) return '';
  const parts = dateStr.split('-');
  if (parts.length !== 3) return dateStr;
  const date = new Date(Date.UTC(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2])));
  if (Number.isNaN(date.getTime())) return dateStr;
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    timeZone: 'UTC',
  }).format(date);
}

const RESOURCE_COLORS = {
  auth: 'teal',
  user: 'indigo',
  users: 'indigo',
  role: 'violet',
  roles: 'violet',
  permission: 'cyan',
  permissions: 'cyan',
  profile: 'blue',
  audit_logs: 'orange',
};

function getResourceColor(resource) {
  return RESOURCE_COLORS[resource] || 'gray';
}

export function ActivityTrendChart({ dailyActivity = [] }) {
  const safeData = Array.isArray(dailyActivity) ? dailyActivity : [];
  const maxTotal = Math.max(1, ...safeData.map((d) => d.total || 0));
  const totalEvents = safeData.reduce((sum, d) => sum + (d.total || 0), 0);

  const chartHeight = 160;
  const topPad = 14;
  const bottomPad = 26;
  const plotHeight = chartHeight - topPad - bottomPad;
  const colWidth = 64;
  const barWidth = 28;
  const svgWidth = Math.max(safeData.length * colWidth, 280);

  const gridTicks = [0, Math.ceil(maxTotal / 2), maxTotal];

  return (
    <Card withBorder radius="md" p="lg" h="100%">
      <Stack gap="md" h="100%">
        <Group justify="space-between" align="flex-start" wrap="wrap" gap="xs">
          <div>
            <Title order={4}>7-Day Security &amp; Audit Activity</Title>
            <Text size="xs" c="dimmed">
              Daily UTC event volume ({totalEvents} total in last 7 days)
            </Text>
          </div>
          <Group gap="xs">
            <Badge size="xs" variant="dot" color="teal">
              Auth
            </Badge>
            <Badge size="xs" variant="dot" color="indigo">
              RBAC / Admin
            </Badge>
            <Badge size="xs" variant="dot" color="red">
              Alerts
            </Badge>
            <Badge size="xs" variant="dot" color="gray">
              Other
            </Badge>
          </Group>
        </Group>

        {safeData.length === 0 ? (
          <Center py="xl">
            <Text size="sm" c="dimmed">
              No activity data available yet.
            </Text>
          </Center>
        ) : (
          <Box style={{ width: '100%', overflowX: 'auto' }}>
            <svg
              role="img"
              aria-label={`7-day audit activity bar chart showing ${totalEvents} total events`}
              viewBox={`0 0 ${svgWidth} ${chartHeight}`}
              width="100%"
              height={chartHeight}
              style={{ display: 'block', minWidth: 280 }}
            >
              {gridTicks.map((tick, idx) => {
                const y = topPad + plotHeight - (tick / maxTotal) * plotHeight;
                return (
                  <g key={`${tick}-${idx}`}>
                    <line
                      x1={0}
                      x2={svgWidth}
                      y1={y}
                      y2={y}
                      stroke="var(--mantine-color-default-border)"
                      strokeDasharray={tick === 0 ? undefined : '3 3'}
                      strokeWidth={1}
                    />
                  </g>
                );
              })}

              {safeData.map((point, index) => {
                const total = point.total || 0;
                const authSuccess = point.auth_success || 0;
                const adminChanges = point.admin_changes || 0;
                const authFailed = point.auth_failed || 0;
                const other = Math.max(0, total - authSuccess - adminChanges - authFailed);

                const xCenter = index * colWidth + colWidth / 2;
                const xBar = xCenter - barWidth / 2;
                const baseY = topPad + plotHeight;

                const segments = [
                  { key: 'auth', value: authSuccess, fill: 'var(--mantine-color-teal-6)' },
                  { key: 'admin', value: adminChanges, fill: 'var(--mantine-color-indigo-6)' },
                  { key: 'other', value: other, fill: 'var(--mantine-color-gray-5)' },
                  { key: 'failed', value: authFailed, fill: 'var(--mantine-color-red-6)' },
                ];

                let currentY = baseY;
                const tooltipText = `${formatShortUtcDate(point.date)}: ${total} total (${authSuccess} auth, ${adminChanges} admin, ${authFailed} alerts, ${other} other)`;

                return (
                  <Tooltip key={point.date} label={tooltipText} withArrow position="top">
                    <g style={{ cursor: 'pointer' }}>
                      <rect
                        x={index * colWidth + 4}
                        y={topPad}
                        width={colWidth - 8}
                        height={plotHeight}
                        fill="transparent"
                      />
                      {total === 0 ? (
                        <rect
                          x={xBar}
                          y={baseY - 2}
                          width={barWidth}
                          height={2}
                          rx={1}
                          fill="var(--mantine-color-default-border)"
                        />
                      ) : (
                        segments.map((seg) => {
                          if (seg.value <= 0) return null;
                          const segHeight = Math.max(2, (seg.value / maxTotal) * plotHeight);
                          currentY -= segHeight;
                          return (
                            <rect
                              key={seg.key}
                              x={xBar}
                              y={currentY}
                              width={barWidth}
                              height={segHeight}
                              rx={2}
                              fill={seg.fill}
                            />
                          );
                        })
                      )}
                      {total > 0 && (
                        <text
                          x={xCenter}
                          y={Math.max(10, currentY - 4)}
                          textAnchor="middle"
                          fontSize="10"
                          fontWeight="600"
                          fill="var(--mantine-color-text)"
                        >
                          {total}
                        </text>
                      )}
                      <text
                        x={xCenter}
                        y={chartHeight - 6}
                        textAnchor="middle"
                        fontSize="11"
                        fill="var(--mantine-color-dimmed)"
                      >
                        {formatShortUtcDate(point.date)}
                      </text>
                    </g>
                  </Tooltip>
                );
              })}
            </svg>
          </Box>
        )}
      </Stack>
    </Card>
  );
}

export function UserStatusDonutChart({ users }) {
  const total = users?.total || 0;
  const statuses = [
    { label: 'Active', count: users?.active || 0, color: 'green' },
    { label: 'Disabled', count: users?.disabled || 0, color: 'red' },
    { label: 'Locked', count: users?.locked || 0, color: 'orange' },
    { label: 'Pending verification', count: users?.pending_verification || 0, color: 'yellow' },
  ];

  const activePct = total > 0 ? Math.round(((users?.active || 0) / total) * 100) : 0;
  const sections =
    total > 0
      ? statuses
          .filter((s) => s.count > 0)
          .map((s) => ({
            value: (s.count / total) * 100,
            color: s.color,
            tooltip: `${s.label}: ${s.count} (${Math.round((s.count / total) * 100)}%)`,
          }))
      : [{ value: 100, color: 'gray' }];

  return (
    <Card withBorder radius="md" p="lg" h="100%">
      <Stack gap="md" h="100%">
        <div>
          <Title order={4}>User Account Status</Title>
          <Text size="xs" c="dimmed">
            Distribution across {total} total {total === 1 ? 'account' : 'accounts'}
          </Text>
        </div>

        <Group align="center" justify="space-between" wrap="wrap" gap="lg" style={{ flex: 1 }}>
          <RingProgress
            size={152}
            thickness={16}
            roundCaps
            sections={sections}
            aria-label={`User status donut chart showing ${activePct}% active accounts`}
            label={
              <Stack gap={0} align="center">
                <Text fw={700} size="lg" lh={1.1}>
                  {activePct}%
                </Text>
                <Text size="xs" c="dimmed">
                  Active
                </Text>
              </Stack>
            }
          />

          <Stack gap="xs" style={{ flex: 1, minWidth: 160 }}>
            {statuses.map((item) => {
              const pct = total > 0 ? Math.round((item.count / total) * 100) : 0;
              return (
                <Group key={item.label} justify="space-between" wrap="nowrap" gap="xs">
                  <Group gap="xs" wrap="nowrap">
                    <Badge size="xs" variant="dot" color={item.color}>
                      {item.label}
                    </Badge>
                  </Group>
                  <Text size="sm" fw={600}>
                    {item.count}{' '}
                    <Text span size="xs" c="dimmed" fw={400}>
                      ({pct}%)
                    </Text>
                  </Text>
                </Group>
              );
            })}
          </Stack>
        </Group>
      </Stack>
    </Card>
  );
}

export function RoleDistributionCard({ roles }) {
  const items = roles?.distribution || [];
  const hasUserCounts = items.some((r) => r.user_count !== undefined && r.user_count !== null);
  const maxVal = Math.max(
    1,
    ...items.map((r) => (hasUserCounts ? r.user_count || 0 : r.permission_count || 0))
  );

  return (
    <Card withBorder radius="md" p="lg" h="100%">
      <Stack gap="md">
        <Group justify="space-between" align="flex-start">
          <div>
            <Title order={4}>Role Coverage &amp; Distribution</Title>
            <Text size="xs" c="dimmed">
              {roles?.system || 0} system · {roles?.custom || 0} custom roles
            </Text>
          </div>
          <Badge variant="light" color="violet">
            {roles?.total || 0} total
          </Badge>
        </Group>

        {items.length === 0 ? (
          <Text size="sm" c="dimmed">
            No roles configured.
          </Text>
        ) : (
          <Stack gap="sm">
            {items.slice(0, 8).map((role) => {
              const metric = hasUserCounts ? role.user_count || 0 : role.permission_count || 0;
              const pct = maxVal > 0 ? Math.max(metric > 0 ? 6 : 0, (metric / maxVal) * 100) : 0;
              return (
                <Stack key={role.id} gap={4}>
                  <Group justify="space-between" wrap="nowrap" gap="xs">
                    <Group gap="xs" wrap="nowrap" style={{ minWidth: 0 }}>
                      <Text size="sm" fw={500} truncate>
                        {role.name}
                      </Text>
                      <Badge size="xs" variant="light" color={role.is_system ? 'indigo' : 'teal'}>
                        {role.is_system ? 'System' : 'Custom'}
                      </Badge>
                    </Group>
                    <Group gap="xs" wrap="nowrap">
                      {hasUserCounts && (
                        <Text size="xs" fw={600}>
                          {role.user_count} {role.user_count === 1 ? 'user' : 'users'}
                        </Text>
                      )}
                      <Text size="xs" c="dimmed">
                        {role.permission_count} perms
                      </Text>
                    </Group>
                  </Group>
                  <Progress
                    value={pct}
                    size="sm"
                    radius="xl"
                    color={role.is_system ? 'violet' : 'teal'}
                    aria-label={`${role.name} distribution bar`}
                  />
                </Stack>
              );
            })}
          </Stack>
        )}
      </Stack>
    </Card>
  );
}

export function ResourceBreakdownCard({ title, subtitle, items = [], badgeColor = 'teal' }) {
  const total = items.reduce((sum, item) => sum + (item.count || 0), 0);
  const maxCount = Math.max(1, ...items.map((item) => item.count || 0));

  return (
    <Card withBorder radius="md" p="lg" h="100%">
      <Stack gap="md">
        <Group justify="space-between" align="flex-start">
          <div>
            <Title order={4}>{title}</Title>
            <Text size="xs" c="dimmed">
              {subtitle}
            </Text>
          </div>
          <Badge variant="light" color={badgeColor}>
            {total} total
          </Badge>
        </Group>

        {items.length === 0 ? (
          <Text size="sm" c="dimmed">
            No data recorded yet.
          </Text>
        ) : (
          <Stack gap="sm">
            {items.slice(0, 8).map((item) => {
              const pct = maxCount > 0 ? Math.max(6, (item.count / maxCount) * 100) : 0;
              const share = total > 0 ? Math.round((item.count / total) * 100) : 0;
              return (
                <Stack key={item.resource} gap={4}>
                  <Group justify="space-between" wrap="nowrap">
                    <Text size="sm" fw={500}>
                      {capitalize(item.resource)}
                    </Text>
                    <Text size="xs" fw={600}>
                      {item.count}{' '}
                      <Text span c="dimmed" fw={400}>
                        ({share}%)
                      </Text>
                    </Text>
                  </Group>
                  <Progress
                    value={pct}
                    size="sm"
                    radius="xl"
                    color={getResourceColor(item.resource)}
                    aria-label={`${item.resource} count ${item.count}`}
                  />
                </Stack>
              );
            })}
          </Stack>
        )}
      </Stack>
    </Card>
  );
}
