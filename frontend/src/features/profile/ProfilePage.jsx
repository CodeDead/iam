import { useEffect, useState } from 'react';
import {
  Badge,
  Button,
  Card,
  Divider,
  Group,
  Modal,
  PasswordInput,
  SimpleGrid,
  Stack,
  Text,
  Textarea,
  TextInput,
  Title,
} from '@mantine/core';
import { useForm } from '@mantine/form';
import { notifications } from '@mantine/notifications';
import { IconTrash } from '@tabler/icons-react';
import { useNavigate } from 'react-router-dom';
import { ErrorAlert } from '../../components/common/ErrorAlert';
import { PageHeader } from '../../components/common/PageHeader';
import { PermissionGuard } from '../../components/common/PermissionGuard';
import { StatusBadge } from '../../components/common/StatusBadge';
import { useAuth } from '../../hooks/useAuth';
import { usePermissions } from '../../hooks/usePermissions';
import { profileService } from '../../services/profileService';
import { extractApiError } from '../../utils/errors';
import { formatDateTime } from '../../utils/formatters';

const PASSWORD_REGEX = /^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[^A-Za-z0-9\s]).{8,128}$/;

export function ProfilePage() {
  const { user, permissions, updateUser, logout, endSession } = useAuth();
  const { hasAnyPermission } = usePermissions();
  const navigate = useNavigate();
  const [profileError, setProfileError] = useState(null);
  const [passwordError, setPasswordError] = useState(null);
  const [deleteError, setDeleteError] = useState(null);
  const [savingProfile, setSavingProfile] = useState(false);
  const [savingPassword, setSavingPassword] = useState(false);
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [deletingAccount, setDeletingAccount] = useState(false);
  const canViewAccessSummary = hasAnyPermission(['roles.read', 'permissions.read']);

  const profileForm = useForm({
    initialValues: {
      first_name: user?.first_name || '',
      last_name: user?.last_name || '',
      email: user?.email || '',
      phone: user?.profile?.phone || '',
      job_title: user?.profile?.job_title || '',
      department: user?.profile?.department || '',
      bio: user?.profile?.bio || '',
    },
    validate: {
      first_name: (v) => (v.trim().length > 0 ? null : 'First name is required'),
      last_name: (v) => (v.trim().length > 0 ? null : 'Last name is required'),
      email: (v) => (/^\S+@\S+\.\S+$/.test(v.trim()) ? null : 'Enter a valid email address'),
    },
  });

  const passwordForm = useForm({
    initialValues: {
      current_password: '',
      new_password: '',
      new_password_confirmation: '',
    },
    validate: {
      current_password: (v) => (v.length > 0 ? null : 'Current password is required'),
      new_password: (v) =>
        PASSWORD_REGEX.test(v)
          ? null
          : 'Min 8 chars with uppercase, lowercase, digit, and special character',
      new_password_confirmation: (v, vals) =>
        v === vals.new_password ? null : 'Password confirmation does not match',
    },
  });

  const deleteForm = useForm({
    initialValues: {
      current_password: '',
    },
    validate: {
      current_password: (v) => (v.length > 0 ? null : 'Current password is required'),
    },
  });

  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const fresh = await profileService.getProfile();
        if (active && fresh) {
          updateUser(fresh);
          profileForm.setValues({
            first_name: fresh.first_name || '',
            last_name: fresh.last_name || '',
            email: fresh.email || '',
            phone: fresh.profile?.phone || '',
            job_title: fresh.profile?.job_title || '',
            department: fresh.profile?.department || '',
            bio: fresh.profile?.bio || '',
          });
        }
      } catch (err) {
        if (active) setProfileError(extractApiError(err, 'Failed to load profile'));
      }
    })();
    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleSaveProfile = async (values) => {
    setProfileError(null);
    setSavingProfile(true);
    try {
      const updated = await profileService.updateProfile({
        first_name: values.first_name.trim(),
        last_name: values.last_name.trim(),
        email: values.email.trim(),
        profile: {
          phone: values.phone.trim() || null,
          job_title: values.job_title.trim() || null,
          department: values.department.trim() || null,
          bio: values.bio.trim() || null,
        },
      });
      updateUser(updated);
      notifications.show({
        title: 'Profile updated',
        message: 'Your account details were saved.',
        color: 'green',
      });
    } catch (err) {
      setProfileError(extractApiError(err, 'Failed to update profile'));
    } finally {
      setSavingProfile(false);
    }
  };

  const handleChangePassword = async (values) => {
    setPasswordError(null);
    setSavingPassword(true);
    try {
      await profileService.changePassword(values);
      notifications.show({
        title: 'Password updated',
        message: 'For your security, please sign in again with your new password.',
        color: 'green',
      });
      passwordForm.reset();
      await logout();
      navigate('/login', { replace: true, state: null });
    } catch (err) {
      setPasswordError(extractApiError(err, 'Failed to change password'));
    } finally {
      setSavingPassword(false);
    }
  };

  const handleCloseDeleteModal = () => {
    if (deletingAccount) return;
    setDeleteModalOpen(false);
    setDeleteError(null);
    deleteForm.reset();
  };

  const handleDeleteAccount = async (values) => {
    setDeleteError(null);
    setDeletingAccount(true);
    try {
      await profileService.deleteAccount(values.current_password);
      notifications.show({
        title: 'Account deleted',
        message: 'Your account has been permanently deleted.',
        color: 'green',
      });
      setDeleteModalOpen(false);
      deleteForm.reset();
      endSession();
      navigate('/login', { replace: true, state: null });
    } catch (err) {
      setDeleteError(extractApiError(err, 'Failed to delete account'));
    } finally {
      setDeletingAccount(false);
    }
  };

  return (
    <Stack gap="lg">
      <PageHeader
        title="My Profile"
        description={
          canViewAccessSummary
            ? 'Manage your personal details, password, and inspect your assigned access'
            : 'Manage your personal details and password'
        }
      />

      <SimpleGrid cols={{ base: 1, lg: 2 }} spacing="lg">
        <Card withBorder radius="md" p="lg">
          <Title order={4} mb="md">
            Personal Information
          </Title>
          <ErrorAlert error={profileError} onClose={() => setProfileError(null)} />
          <form onSubmit={profileForm.onSubmit(handleSaveProfile)}>
            <Stack gap="sm">
              <Group grow>
                <TextInput
                  label="First name"
                  required
                  {...profileForm.getInputProps('first_name')}
                />
                <TextInput label="Last name" required {...profileForm.getInputProps('last_name')} />
              </Group>
              <TextInput label="Email address" required {...profileForm.getInputProps('email')} />
              <Group grow>
                <TextInput label="Job title" {...profileForm.getInputProps('job_title')} />
                <TextInput label="Department" {...profileForm.getInputProps('department')} />
              </Group>
              <TextInput label="Phone" {...profileForm.getInputProps('phone')} />
              <Textarea label="Bio" minRows={3} {...profileForm.getInputProps('bio')} />
              <Group justify="flex-end" mt="sm">
                <Button type="submit" loading={savingProfile}>
                  Save changes
                </Button>
              </Group>
            </Stack>
          </form>
        </Card>

        <Stack gap="lg">
          <Card withBorder radius="md" p="lg">
            <Title order={4} mb="md">
              Change Password
            </Title>
            <ErrorAlert error={passwordError} onClose={() => setPasswordError(null)} />
            <form onSubmit={passwordForm.onSubmit(handleChangePassword)}>
              <Stack gap="sm">
                <PasswordInput
                  label="Current password"
                  required
                  {...passwordForm.getInputProps('current_password')}
                />
                <PasswordInput
                  label="New password"
                  required
                  {...passwordForm.getInputProps('new_password')}
                />
                <PasswordInput
                  label="Confirm new password"
                  required
                  {...passwordForm.getInputProps('new_password_confirmation')}
                />
                <Group justify="flex-end" mt="sm">
                  <Button type="submit" color="orange" loading={savingPassword}>
                    Update password
                  </Button>
                </Group>
              </Stack>
            </form>
          </Card>

          <PermissionGuard anyPermissions={['roles.read', 'permissions.read']}>
            <Card withBorder radius="md" p="lg">
              <Title order={4} mb="sm">
                Account Access Summary
              </Title>
              <Stack gap="xs">
                <Group justify="space-between">
                  <Text size="sm" c="dimmed">
                    Status
                  </Text>
                  <StatusBadge status={user?.status} />
                </Group>
                <Group justify="space-between">
                  <Text size="sm" c="dimmed">
                    Last login
                  </Text>
                  <Text size="sm">{formatDateTime(user?.last_login_at)}</Text>
                </Group>
                <Divider my="xs" />
                <Text size="sm" fw={600}>
                  Assigned Roles
                </Text>
                <Group gap="xs">
                  {(user?.roles || []).map((role) => (
                    <Badge key={role.id} color="indigo">
                      {role.name}
                    </Badge>
                  ))}
                </Group>
                <Text size="sm" fw={600} mt="xs">
                  Effective Permissions ({permissions.length})
                </Text>
                <Group gap="xs">
                  {permissions.map((perm) => (
                    <Badge key={perm} variant="light" color="gray">
                      {perm}
                    </Badge>
                  ))}
                </Group>
              </Stack>
            </Card>
          </PermissionGuard>

          <PermissionGuard permission="profile.delete">
            <Card
              withBorder
              radius="md"
              p="lg"
              style={{ borderColor: 'var(--mantine-color-red-5)' }}
            >
              <Stack gap="sm">
                <Title order={4} c="red">
                  Delete Account
                </Title>
                <Text size="sm" c="dimmed">
                  Permanently delete your account and revoke all active sessions. This action cannot
                  be undone.
                </Text>
                <Group justify="flex-end">
                  <Button
                    color="red"
                    variant="light"
                    leftSection={<IconTrash size={16} />}
                    onClick={() => {
                      setDeleteError(null);
                      deleteForm.reset();
                      setDeleteModalOpen(true);
                    }}
                  >
                    Delete my account
                  </Button>
                </Group>
              </Stack>
            </Card>
          </PermissionGuard>
        </Stack>
      </SimpleGrid>

      <Modal
        opened={deleteModalOpen}
        onClose={handleCloseDeleteModal}
        title="Confirm Account Deletion"
        centered
      >
        <form onSubmit={deleteForm.onSubmit(handleDeleteAccount)}>
          <Stack gap="sm">
            <Text size="sm">
              Are you sure you want to permanently delete <strong>{user?.email}</strong>? All active
              sessions will be revoked immediately and your account cannot be recovered.
            </Text>
            <ErrorAlert error={deleteError} onClose={() => setDeleteError(null)} />
            <PasswordInput
              label="Enter your current password to confirm"
              placeholder="Current password"
              required
              {...deleteForm.getInputProps('current_password')}
            />
            <Group justify="flex-end" mt="sm">
              <Button variant="default" onClick={handleCloseDeleteModal} disabled={deletingAccount}>
                Cancel
              </Button>
              <Button type="submit" color="red" loading={deletingAccount}>
                Permanently delete account
              </Button>
            </Group>
          </Stack>
        </form>
      </Modal>
    </Stack>
  );
}
