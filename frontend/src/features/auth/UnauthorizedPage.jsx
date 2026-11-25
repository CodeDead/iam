import { Button, Center, Paper, Stack, Text, ThemeIcon, Title } from '@mantine/core';
import { IconLock } from '@tabler/icons-react';
import { useNavigate } from 'react-router-dom';

const UnauthorizedPage = () => {
  const navigate = useNavigate();
  return (
    <Center component="main" mih="70vh">
      <Paper withBorder p="xl" radius="md" maw={420} w="100%">
        <Stack align="center" ta="center" gap="md">
          <ThemeIcon size={56} radius="xl" color="orange" variant="light">
            <IconLock size={28} />
          </ThemeIcon>
          <Title order={2}>401 — Unauthorized</Title>
          <Text size="sm" c="dimmed">
            Your session has expired or you are not signed in. Please sign in to continue.
          </Text>
          <Button onClick={() => navigate('/login', { replace: true })}>Go to Sign In</Button>
        </Stack>
      </Paper>
    </Center>
  );
};

export default UnauthorizedPage;
