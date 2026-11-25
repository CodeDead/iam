import { useState } from 'react';
import { Button, Code, Group, Modal, Stack, Text, Textarea, TextInput } from '@mantine/core';
import { useForm } from '@mantine/form';
import { notifications } from '@mantine/notifications';
import ErrorAlert from '../../components/common/ErrorAlert';
import { permissionService } from '../../services/permissionService';
import { extractApiError } from '../../utils/errors';

const SEGMENT_REGEX = /^[a-z][a-z0-9_]{0,63}$/;

const PermissionFormModal = ({ opened, onClose, permission, onSaved }) => {
  const isEditing = Boolean(permission);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState(null);

  const form = useForm({
    initialValues: {
      resource: permission?.resource || '',
      action: permission?.action || '',
      description: permission?.description || '',
    },
    validate: {
      resource: (v) =>
        SEGMENT_REGEX.test(v.trim())
          ? null
          : 'Lowercase letters, digits, underscores; must start with a letter',
      action: (v) =>
        SEGMENT_REGEX.test(v.trim())
          ? null
          : 'Lowercase letters, digits, underscores; must start with a letter',
      description: (v) => (v.trim().length > 0 ? null : 'Description is required'),
    },
  });

  const handleSubmit = async (values) => {
    setError(null);
    setSubmitting(true);
    try {
      const payload = {
        resource: values.resource.trim().toLowerCase(),
        action: values.action.trim().toLowerCase(),
        description: values.description.trim(),
      };
      if (isEditing) {
        await permissionService.update(permission.id, payload);
        notifications.show({
          title: 'Permission updated',
          message: `Permission "${payload.resource}.${payload.action}" updated.`,
          color: 'green',
        });
      } else {
        await permissionService.create(payload);
        notifications.show({
          title: 'Permission created',
          message: `Permission "${payload.resource}.${payload.action}" created.`,
          color: 'green',
        });
      }
      onSaved();
    } catch (err) {
      setError(extractApiError(err, 'Failed to save permission'));
    } finally {
      setSubmitting(false);
    }
  };

  const previewName =
    form.values.resource && form.values.action
      ? `${form.values.resource.trim().toLowerCase()}.${form.values.action.trim().toLowerCase()}`
      : 'resource.action';

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={isEditing ? 'Edit Permission' : 'Create Permission'}
      size="md"
    >
      <ErrorAlert error={error} onClose={() => setError(null)} />
      <form onSubmit={form.onSubmit(handleSubmit)}>
        <Stack gap="sm">
          <Group grow>
            <TextInput
              label="Resource"
              placeholder="e.g. reports"
              required
              {...form.getInputProps('resource')}
            />
            <TextInput
              label="Action"
              placeholder="e.g. export"
              required
              {...form.getInputProps('action')}
            />
          </Group>

          <Text size="xs" c="dimmed">
            Permission identifier: <Code>{previewName}</Code>
          </Text>

          <Textarea
            label="Description"
            placeholder="Describe what this permission grants"
            required
            minRows={2}
            {...form.getInputProps('description')}
          />

          <Group justify="flex-end" mt="md">
            <Button variant="default" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={submitting}>
              {isEditing ? 'Save changes' : 'Create permission'}
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
};

export default PermissionFormModal;
