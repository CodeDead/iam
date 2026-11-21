import { useState } from 'react';
import { Button, Group, Modal, MultiSelect, Stack, Text } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { userService } from '../../services/userService';
import { extractApiError } from '../../utils/errors';

export function AssignRolesModal({ user, availableRoles = [], onClose, onSaved }) {
  const [selectedRoleIds, setSelectedRoleIds] = useState((user?.roles || []).map((r) => r.id));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState(null);

  const handleSave = async () => {
    if (!user) return;
    if (selectedRoleIds.length === 0) {
      setError('A user must have at least one assigned role.');
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await userService.updateRoles(user.id, selectedRoleIds);
      notifications.show({
        title: 'Roles updated',
        message: `Assigned roles for ${user.email} have been updated.`,
        color: 'green',
      });
      onSaved();
    } catch (err) {
      setError(extractApiError(err, 'Failed to update roles'));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal opened={Boolean(user)} onClose={onClose} title="Assign Roles" size="md">
      <Stack gap="md">
        <Text size="sm" c="dimmed">
          Select the roles to assign to <strong>{user?.email}</strong>. Changing a user&apos;s roles
          immediately updates their effective permissions and revokes existing refresh sessions.
        </Text>

        <ErrorAlert error={error} onClose={() => setError(null)} />

        <MultiSelect
          label="Roles"
          data={availableRoles.map((r) => ({ value: r.id, label: r.name }))}
          value={selectedRoleIds}
          onChange={setSelectedRoleIds}
          searchable
          required
        />

        <Group justify="flex-end">
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={handleSave} loading={submitting}>
            Save roles
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
