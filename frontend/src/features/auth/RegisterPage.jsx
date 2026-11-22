import { useState } from 'react';
import {
  Anchor,
  Button,
  Group,
  PasswordInput,
  SegmentedControl,
  Stack,
  Text,
  TextInput,
  Title,
} from '@mantine/core';
import { useForm } from '@mantine/form';
import { notifications } from '@mantine/notifications';
import { Link, useNavigate } from 'react-router-dom';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { useAuth } from '../../hooks/useAuth';
import { extractApiError } from '../../utils/errors';

const PASSWORD_REGEX = /^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[^A-Za-z0-9\s]).{8,128}$/;

export function RegisterPage() {
  const { register } = useAuth();
  const navigate = useNavigate();
  const [accountType, setAccountType] = useState('personal');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState(null);

  const form = useForm({
    initialValues: {
      organisation_name: '',
      first_name: '',
      last_name: '',
      email: '',
      password: '',
      password_confirmation: '',
    },
    validate: {
      organisation_name: (v) => {
        if (accountType !== 'organisation') return null;
        const len = v.trim().length;
        return len >= 2 && len <= 120 ? null : 'Organisation name must be 2 to 120 characters';
      },
      first_name: (v) => (v.trim().length > 0 ? null : 'First name is required'),
      last_name: (v) => (v.trim().length > 0 ? null : 'Last name is required'),
      email: (v) => (/^\S+@\S+\.\S+$/.test(v.trim()) ? null : 'Enter a valid email address'),
      password: (v) =>
        PASSWORD_REGEX.test(v)
          ? null
          : 'Min 8 chars with uppercase, lowercase, digit, and special character',
      password_confirmation: (v, values) =>
        v === values.password ? null : 'Passwords do not match',
    },
  });

  const handleSubmit = async (values) => {
    setError(null);
    setSubmitting(true);
    try {
      const payload = {
        first_name: values.first_name.trim(),
        last_name: values.last_name.trim(),
        email: values.email.trim(),
        password: values.password,
        password_confirmation: values.password_confirmation,
      };
      if (accountType === 'organisation') {
        payload.organisation_name = values.organisation_name.trim();
      }
      await register(payload);
      notifications.show({
        title: 'Account created',
        message:
          accountType === 'organisation'
            ? `Welcome! Your account and organisation '${values.organisation_name.trim()}' have been registered.`
            : 'Welcome! Your account has been registered.',
        color: 'green',
      });
      navigate('/', { replace: true });
    } catch (err) {
      setError(extractApiError(err, 'Registration failed'));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Stack gap="md">
      <div>
        <Title order={2}>Create an account</Title>
        <Text size="sm" c="dimmed" mt={4}>
          {accountType === 'organisation'
            ? 'Register an organisation account and become its active owner'
            : 'Register a standard personal user account'}
        </Text>
      </div>

      <SegmentedControl
        fullWidth
        value={accountType}
        onChange={(value) => {
          setAccountType(value);
          form.clearFieldError('organisation_name');
        }}
        data={[
          { label: 'Personal Account', value: 'personal' },
          { label: 'Organisation Account', value: 'organisation' },
        ]}
      />

      <ErrorAlert error={error} onClose={() => setError(null)} />

      <form onSubmit={form.onSubmit(handleSubmit)}>
        <Stack gap="sm">
          {accountType === 'organisation' && (
            <TextInput
              label="Organisation name"
              placeholder="Acme Inc."
              required
              {...form.getInputProps('organisation_name')}
            />
          )}

          <Group grow>
            <TextInput
              label="First name"
              placeholder="Jane"
              required
              {...form.getInputProps('first_name')}
            />
            <TextInput
              label="Last name"
              placeholder="Doe"
              required
              {...form.getInputProps('last_name')}
            />
          </Group>

          <TextInput
            label="Email address"
            placeholder="jane@example.com"
            required
            autoComplete="email"
            {...form.getInputProps('email')}
          />

          <PasswordInput
            label="Password"
            placeholder="Create a strong password"
            description="At least 8 characters including upper, lower, digit, and symbol"
            required
            autoComplete="new-password"
            {...form.getInputProps('password')}
          />

          <PasswordInput
            label="Confirm password"
            placeholder="Repeat your password"
            required
            autoComplete="new-password"
            {...form.getInputProps('password_confirmation')}
          />

          <Button type="submit" fullWidth mt="sm" loading={submitting}>
            {accountType === 'organisation' ? 'Register Organisation' : 'Register'}
          </Button>
        </Stack>
      </form>

      <Text size="sm" ta="center" c="dimmed">
        Already have an account?{' '}
        <Anchor component={Link} to="/login" fw={500}>
          Sign in
        </Anchor>
      </Text>
    </Stack>
  );
}
