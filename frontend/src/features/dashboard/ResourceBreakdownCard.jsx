import { Badge, Card, Group, Progress, Stack, Text, Title } from '@mantine/core';
import { capitalize } from '../../utils/formatters';

const RESOURCE_COLORS = {
  auth: 'teal',
  user: 'indigo',
  users: 'indigo',
  organisation: 'grape',
  organisations: 'grape',
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

const ResourceBreakdownCard = ({ title, subtitle, items = [], badgeColor = 'teal' }) => {
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
};

export default ResourceBreakdownCard;
