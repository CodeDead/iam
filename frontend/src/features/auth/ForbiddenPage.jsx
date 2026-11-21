import { Button, Center, Paper, Stack, Text, ThemeIcon, Title } from '@mantine/core';
import { IconShieldOff } from '@tabler/icons-react';
import { useNavigate } from 'react-router-dom';

export function ForbiddenPage() {
  const navigate = useNavigate();
  return (
    <Center component="main" mih="70vh">
      <Paper withBorder p="xl" radius="md" maw={440} w="100%">
        <Stack align="center" ta="center" gap="md">
          <ThemeIcon size={56} radius="xl" color="red" variant="light">
            <IconShieldOff size={28} />
          </ThemeIcon>
          <Title order={2}>403 — Access Forbidden</Title>
          <Text size="sm" c="dimmed">
            You do not have the required permissions to view this page or perform this action.
          </Text>
          <Button variant="light" onClick={() => navigate('/', { replace: true })}>
            Return to Dashboard
          </Button>
        </Stack>
      </Paper>
    </Center>
  );
}
