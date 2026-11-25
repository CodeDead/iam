import { useState } from 'react';
import { Button, Divider, Group, Modal, Stack, Text, Textarea, TextInput } from '@mantine/core';
import { useForm } from '@mantine/form';
import { notifications } from '@mantine/notifications';
import ErrorAlert from '../../components/common/ErrorAlert';
import PermissionMatrix from '../../components/forms/PermissionMatrix';
import { roleService } from '../../services/roleService';
import { extractApiError } from '../../utils/errors';

const RoleFormModal = ({ opened, onClose, role, allPermissions = [], onSaved, onPartialSaved }) => {
  const isEditing = Boolean(role);
  const [selectedPermissionIds, setSelectedPermissionIds] = useState(role?.permission_ids || []);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState(null);

  const form = useForm({
    initialValues: {
      name: role?.name || '',
      description: role?.description || '',
    },
    validate: {
      name: (v) => (v.trim().length >= 2 ? null : 'Role name must be at least 2 characters'),
      description: (v) => (v.trim().length > 0 ? null : 'Description is required'),
    },
  });

  const handleSubmit = async (values) => {
    setError(null);
    setSubmitting(true);
    let updatedRoleDetails = false;
    try {
      if (isEditing) {
        const updatePayload = { description: values.description.trim() };
        if (!role.is_system) {
          updatePayload.name = values.name.trim();
        }
        await roleService.update(role.id, updatePayload);
        updatedRoleDetails = true;
        await roleService.updatePermissions(role.id, selectedPermissionIds);
        notifications.show({
          title: 'Role updated',
          message: `Role "${role.name}" and its permissions were saved.`,
          color: 'green',
        });
      } else {
        await roleService.create({
          name: values.name.trim(),
          description: values.description.trim(),
          permission_ids: selectedPermissionIds,
        });
        notifications.show({
          title: 'Role created',
          message: `Role "${values.name.trim()}" was created.`,
          color: 'green',
        });
      }
      onSaved();
    } catch (err) {
      if (updatedRoleDetails && onPartialSaved) {
        onPartialSaved();
      }
      setError(extractApiError(err, 'Failed to save role'));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={isEditing ? `Edit Role: ${role?.name}` : 'Create Role'}
      size="xl"
    >
      <ErrorAlert error={error} onClose={() => setError(null)} />
      <form onSubmit={form.onSubmit(handleSubmit)}>
        <Stack gap="md">
          <TextInput
            label="Role name"
            required
            disabled={Boolean(role?.is_system)}
            description={role?.is_system ? 'System role names cannot be changed' : undefined}
            {...form.getInputProps('name')}
          />

          <Textarea
            label="Description"
            required
            minRows={2}
            {...form.getInputProps('description')}
          />

          <Divider label="Assigned Permissions" labelPosition="left" />

          <Text size="xs" c="dimmed">
            Selected: {selectedPermissionIds.length} permission(s)
          </Text>

          <PermissionMatrix
            allPermissions={allPermissions}
            selectedIds={selectedPermissionIds}
            onChange={setSelectedPermissionIds}
          />

          <Group justify="flex-end" mt="md">
            <Button variant="default" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={submitting}>
              {isEditing ? 'Save role' : 'Create role'}
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
};

export default RoleFormModal;
