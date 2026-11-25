import { Badge, Card, Group, RingProgress, Stack, Text, Title } from '@mantine/core';

const UserStatusDonutChart = ({ users }) => {
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
};

export default UserStatusDonutChart;
