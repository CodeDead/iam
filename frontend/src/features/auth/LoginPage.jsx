import { useState } from 'react';
import { Anchor, Button, PasswordInput, Stack, Text, TextInput, Title } from '@mantine/core';
import { useForm } from '@mantine/form';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { useAuth } from '../../hooks/useAuth';
import { extractApiError } from '../../utils/errors';

const ROUTE_PERMISSIONS = {
  '/users': 'users.read',
  '/roles': 'roles.read',
  '/permissions': 'permissions.read',
  '/profile': 'profile.read',
};

function resolvePostLoginRedirect(fromPath, permissions = []) {
  if (!fromPath || fromPath === '/login' || fromPath === '/register' || fromPath === '/forbidden') {
    return '/';
  }
  const required = Object.entries(ROUTE_PERMISSIONS).find(([prefix]) =>
    fromPath.startsWith(prefix)
  )?.[1];
  if (required && !permissions.includes(required)) {
    return '/';
  }
  return fromPath;
}

export function LoginPage() {
  const { login } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState(null);

  const form = useForm({
    initialValues: {
      email: '',
      password: '',
    },
    validate: {
      email: (v) => (/^\S+@\S+\.\S+$/.test(v.trim()) ? null : 'Enter a valid email address'),
      password: (v) => (v.length > 0 ? null : 'Password is required'),
    },
  });

  const handleSubmit = async (values) => {
    setError(null);
    setSubmitting(true);
    try {
      const session = await login({
        email: values.email.trim(),
        password: values.password,
      });
      const redirectTo = resolvePostLoginRedirect(
        location.state?.from?.pathname,
        session?.permissions || []
      );
      navigate(redirectTo, { replace: true });
    } catch (err) {
      setError(extractApiError(err, 'Unable to sign in'));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Stack gap="md">
      <div>
        <Title order={2}>Sign in</Title>
        <Text size="sm" c="dimmed" mt={4}>
          Enter your credentials to access your account
        </Text>
      </div>

      <ErrorAlert error={error} onClose={() => setError(null)} />

      <form onSubmit={form.onSubmit(handleSubmit)}>
        <Stack gap="sm">
          <TextInput
            label="Email address"
            placeholder="you@example.com"
            required
            autoComplete="email"
            {...form.getInputProps('email')}
          />

          <PasswordInput
            label="Password"
            placeholder="Your password"
            required
            autoComplete="current-password"
            {...form.getInputProps('password')}
          />

          <Button type="submit" fullWidth mt="sm" loading={submitting}>
            Sign in
          </Button>
        </Stack>
      </form>

      <Text size="sm" ta="center" c="dimmed">
        Don&apos;t have an account?{' '}
        <Anchor component={Link} to="/register" fw={500}>
          Create one
        </Anchor>
      </Text>
    </Stack>
  );
}
