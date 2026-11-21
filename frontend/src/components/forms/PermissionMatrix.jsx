import { Checkbox, Group, Paper, SimpleGrid, Stack, Text } from '@mantine/core';
import { capitalize, groupPermissionsByResource } from '../../utils/formatters';

export function PermissionMatrix({
  allPermissions = [],
  selectedIds = [],
  onChange,
  disabled = false,
}) {
  const groups = groupPermissionsByResource(allPermissions);
  const selectedSet = new Set(selectedIds);

  const togglePermission = (id) => {
    const next = new Set(selectedSet);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    onChange(Array.from(next));
  };

  const toggleGroup = (items) => {
    const ids = items.map((p) => p.id);
    const allSelected = ids.every((id) => selectedSet.has(id));
    const next = new Set(selectedSet);
    if (allSelected) {
      ids.forEach((id) => next.delete(id));
    } else {
      ids.forEach((id) => next.add(id));
    }
    onChange(Array.from(next));
  };

  if (groups.length === 0) {
    return (
      <Text size="sm" c="dimmed">
        No permissions available.
      </Text>
    );
  }

  return (
    <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="md">
      {groups.map(({ resource, items }) => {
        const allChecked = items.every((p) => selectedSet.has(p.id));
        const someChecked = items.some((p) => selectedSet.has(p.id)) && !allChecked;

        return (
          <Paper key={resource} withBorder p="md" radius="md">
            <Group justify="space-between" mb="sm">
              <Checkbox
                label={
                  <Text fw={600} size="sm">
                    {capitalize(resource)}
                  </Text>
                }
                checked={allChecked}
                indeterminate={someChecked}
                disabled={disabled}
                onChange={() => toggleGroup(items)}
              />
              <Text size="xs" c="dimmed">
                {items.filter((p) => selectedSet.has(p.id)).length}/{items.length}
              </Text>
            </Group>
            <Stack gap="xs" pl="md">
              {items.map((perm) => (
                <Checkbox
                  key={perm.id}
                  label={capitalize(perm.action)}
                  description={perm.description}
                  checked={selectedSet.has(perm.id)}
                  disabled={disabled}
                  onChange={() => togglePermission(perm.id)}
                />
              ))}
            </Stack>
          </Paper>
        );
      })}
    </SimpleGrid>
  );
}
