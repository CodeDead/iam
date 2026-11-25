import { Badge, Card, Group, Progress, Stack, Text, Title } from '@mantine/core';

const RoleDistributionCard = ({ roles }) => {
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
};

export default RoleDistributionCard;
