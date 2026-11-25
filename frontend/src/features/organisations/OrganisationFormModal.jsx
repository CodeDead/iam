import { useState } from 'react';
import { Button, Checkbox, Group, Modal, Stack, TextInput } from '@mantine/core';
import { useForm } from '@mantine/form';
import { notifications } from '@mantine/notifications';
import ErrorAlert from '../../components/common/ErrorAlert';
import { usePermissions } from '../../hooks/usePermissions';
import { organisationService } from '../../services/organisationService';
import { extractApiError } from '../../utils/errors';

const EMAIL_REGEX = /^\S+@\S+\.\S+$/;

const OrganisationFormModal = ({ opened, onClose, organisation = null, onSaved }) => {
  const { hasPermission } = usePermissions();
  const canConfigureCreator = hasPermission('organisations.manage_members');
  const isEdit = Boolean(organisation);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState(null);

  const form = useForm({
    initialValues: {
      name: organisation?.name || '',
      include_creator: true,
      initial_owner_email: '',
    },
    validate: {
      name: (v) =>
        v.trim().length >= 2 && v.trim().length <= 120
          ? null
          : 'Organisation name must be 2 to 120 characters',
      initial_owner_email: (v, values) => {
        if (isEdit || !canConfigureCreator || values.include_creator) return null;
        return EMAIL_REGEX.test(v.trim())
          ? null
          : 'Enter a valid email address for the initial active owner';
      },
    },
  });

  const handleSubmit = async (values) => {
    setError(null);
    setSubmitting(true);
    try {
      if (isEdit) {
        await organisationService.update(organisation.id, { name: values.name.trim() });
        notifications.show({
          title: 'Organisation updated',
          message: `Updated organisation '${values.name.trim()}'.`,
          color: 'green',
        });
      } else {
        const payload = { name: values.name.trim() };
        if (canConfigureCreator) {
          payload.include_creator = values.include_creator;
          if (!values.include_creator && values.initial_owner_email.trim()) {
            payload.initial_owner_email = values.initial_owner_email.trim();
          }
        }
        await organisationService.create(payload);
        notifications.show({
          title: 'Organisation created',
          message: `Created organisation '${values.name.trim()}'.`,
          color: 'green',
        });
      }
      onSaved();
      onClose();
    } catch (err) {
      setError(extractApiError(err, 'Failed to save organisation'));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={isEdit ? `Edit Organisation: ${organisation?.name}` : 'Create Organisation'}
      size="md"
    >
      <form onSubmit={form.onSubmit(handleSubmit)}>
        <Stack gap="sm">
          <ErrorAlert error={error} onClose={() => setError(null)} />

          <TextInput
            label="Organisation name"
            placeholder="Acme Inc."
            required
            {...form.getInputProps('name')}
          />

          {!isEdit && canConfigureCreator && (
            <>
              <Checkbox
                label="Add myself as an active owner"
                checked={form.values.include_creator}
                onChange={(event) => {
                  const checked = event.currentTarget.checked;
                  form.setFieldValue('include_creator', checked);
                  if (checked) {
                    form.clearFieldError('initial_owner_email');
                  }
                }}
              />
              {!form.values.include_creator && (
                <TextInput
                  type="email"
                  label="Initial owner email"
                  description="Assign an existing user as the initial active owner of this organisation"
                  placeholder="owner@example.com"
                  required
                  {...form.getInputProps('initial_owner_email')}
                />
              )}
            </>
          )}

          <Group justify="flex-end" mt="md">
            <Button variant="default" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={submitting}>
              {isEdit ? 'Save changes' : 'Create organisation'}
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
};

export default OrganisationFormModal;
