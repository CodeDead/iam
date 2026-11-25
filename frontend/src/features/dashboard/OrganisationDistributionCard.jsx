import { Badge, Card, Group, Progress, Stack, Text, Title } from '@mantine/core';

const OrganisationDistributionCard = ({ organisations }) => {
  const items = organisations?.largest_organisations || [];
  const maxMembers = Math.max(1, ...items.map((org) => org.member_count || 0));

  return (
    <Card withBorder radius="md" p="lg" h="100%">
      <Stack gap="md">
        <Group justify="space-between" align="flex-start">
          <div>
            <Title order={4}>Top Organisations by Members</Title>
            <Text size="xs" c="dimmed">
              {organisations?.active_memberships || 0} active ·{' '}
              {organisations?.invited_memberships || 0} invited ·{' '}
              {organisations?.suspended_memberships || 0} suspended
            </Text>
          </div>
          <Badge variant="light" color="grape">
            {organisations?.total || 0} orgs
          </Badge>
        </Group>

        {items.length === 0 ? (
          <Text size="sm" c="dimmed">
            No organisations created yet.
          </Text>
        ) : (
          <Stack gap="sm">
            {items.slice(0, 8).map((org) => {
              const count = org.member_count || 0;
              const pct =
                maxMembers > 0 ? Math.max(count > 0 ? 6 : 0, (count / maxMembers) * 100) : 0;
              return (
                <Stack key={org.id} gap={4}>
                  <Group justify="space-between" wrap="nowrap" gap="xs">
                    <Text size="sm" fw={500} truncate>
                      {org.name}
                    </Text>
                    <Group gap="xs" wrap="nowrap">
                      <Text size="xs" fw={600}>
                        {org.active_member_count} active
                      </Text>
                      <Text size="xs" c="dimmed">
                        / {count} total
                      </Text>
                    </Group>
                  </Group>
                  <Progress
                    value={pct}
                    size="sm"
                    radius="xl"
                    color="grape"
                    aria-label={`${org.name} has ${count} members`}
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

export default OrganisationDistributionCard;
