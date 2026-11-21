import { useState } from 'react';
import {
  Button,
  Group,
  Modal,
  MultiSelect,
  PasswordInput,
  Select,
  Stack,
  Switch,
  TextInput,
} from '@mantine/core';
import { useForm } from '@mantine/form';
import { notifications } from '@mantine/notifications';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { userService } from '../../services/userService';
import { extractApiError } from '../../utils/errors';

const PASSWORD_REGEX = /^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[^A-Za-z0-9\s]).{8,128}$/;

export function UserFormModal({
  opened,
  onClose,
  user,
  availableRoles = [],
  canAssignRoles = false,
  onSaved,
}) {
  const isEditing = Boolean(user);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState(null);

  const form = useForm({
    initialValues: {
      first_name: user?.first_name || '',
      last_name: user?.last_name || '',
      email: user?.email || '',
      password: '',
      status: user?.status || 'active',
      email_verified: Boolean(user?.email_verified),
      role_ids: (user?.roles || []).map((r) => r.id),
    },
    validate: {
      first_name: (v) => (v.trim().length > 0 ? null : 'First name is required'),
      last_name: (v) => (v.trim().length > 0 ? null : 'Last name is required'),
      email: (v) => (/^\S+@\S+\.\S+$/.test(v.trim()) ? null : 'Valid email is required'),
      password: (v) => {
        if (isEditing) return null;
        return PASSWORD_REGEX.test(v)
          ? null
          : 'Min 8 chars with uppercase, lowercase, digit, and symbol';
      },
    },
  });

  const handleSubmit = async (values) => {
    setError(null);
    setSubmitting(true);
    try {
      if (isEditing) {
        await userService.update(user.id, {
          first_name: values.first_name.trim(),
          last_name: values.last_name.trim(),
          email: values.email.trim(),
          email_verified: values.email_verified,
        });
        notifications.show({
          title: 'User updated',
          message: 'Account information saved.',
          color: 'green',
        });
      } else {
        const payload = {
          first_name: values.first_name.trim(),
          last_name: values.last_name.trim(),
          email: values.email.trim(),
          password: values.password,
          status: values.status,
          email_verified: values.email_verified,
        };
        if (canAssignRoles && values.role_ids.length > 0) {
          payload.role_ids = values.role_ids;
        }
        await userService.create(payload);
        notifications.show({
          title: 'User created',
          message: 'New user account has been created.',
          color: 'green',
        });
      }
      onSaved();
    } catch (err) {
      setError(extractApiError(err, 'Failed to save user'));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={isEditing ? 'Edit User' : 'Create User'}
      size="md"
    >
      <ErrorAlert error={error} onClose={() => setError(null)} />
      <form onSubmit={form.onSubmit(handleSubmit)}>
        <Stack gap="sm">
          <Group grow>
            <TextInput label="First name" required {...form.getInputProps('first_name')} />
            <TextInput label="Last name" required {...form.getInputProps('last_name')} />
          </Group>

          <TextInput label="Email address" required {...form.getInputProps('email')} />

          {!isEditing && (
            <>
              <PasswordInput
                label="Initial password"
                required
                description="Min 8 chars with uppercase, lowercase, digit, and special character"
                {...form.getInputProps('password')}
              />
              <Select
                label="Account status"
                data={[
                  { value: 'active', label: 'Active' },
                  { value: 'pending_verification', label: 'Pending Verification' },
                  { value: 'disabled', label: 'Disabled' },
                ]}
                {...form.getInputProps('status')}
              />
              {canAssignRoles && availableRoles.length > 0 && (
                <MultiSelect
                  label="Initial roles"
                  placeholder="Defaults to User role if omitted"
                  data={availableRoles.map((r) => ({ value: r.id, label: r.name }))}
                  searchable
                  {...form.getInputProps('role_ids')}
                />
              )}
            </>
          )}

          <Switch
            label="Email verified"
            mt="xs"
            {...form.getInputProps('email_verified', { type: 'checkbox' })}
          />

          <Group justify="flex-end" mt="md">
            <Button variant="default" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={submitting}>
              {isEditing ? 'Save changes' : 'Create user'}
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
}
