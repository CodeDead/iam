import { Box, Center, Container, Group, Paper, Text, ThemeIcon } from '@mantine/core';
import { IconShieldCheck } from '@tabler/icons-react';
import { Outlet } from 'react-router-dom';

export function AuthLayout() {
  return (
    <Box mih="100vh" bg="gray.0" py="xl">
      <Center mih="calc(100vh - 64px)">
        <Container size={440} w="100%">
          <Group justify="center" mb="lg" gap="xs">
            <ThemeIcon size="lg" radius="md" color="indigo">
              <IconShieldCheck size={22} />
            </ThemeIcon>
            <Text fw={700} size="xl">
              CodeDead IAM
            </Text>
          </Group>
          <Paper withBorder shadow="md" p="xl" radius="md">
            <Outlet />
          </Paper>
        </Container>
      </Center>
    </Box>
  );
}
