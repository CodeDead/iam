import {
  ActionIcon,
  Box,
  Center,
  Container,
  Group,
  Paper,
  Text,
  ThemeIcon,
  Tooltip,
  useComputedColorScheme,
  useMantineColorScheme,
} from '@mantine/core';
import { IconMoon, IconShieldCheck, IconSun } from '@tabler/icons-react';
import { Outlet } from 'react-router-dom';

export function AuthLayout() {
  const { setColorScheme } = useMantineColorScheme();
  const computedColorScheme = useComputedColorScheme('light', { getInitialValueInEffect: true });

  const toggleColorScheme = () => setColorScheme(computedColorScheme === 'dark' ? 'light' : 'dark');

  return (
    <Box mih="100vh" bg="var(--mantine-color-body)" py="md" px="md">
      <Group component="header" justify="flex-end">
        <Tooltip label="Toggle color scheme">
          <ActionIcon
            variant="default"
            size="lg"
            onClick={toggleColorScheme}
            aria-label="Toggle color scheme"
          >
            {computedColorScheme === 'dark' ? <IconSun size={18} /> : <IconMoon size={18} />}
          </ActionIcon>
        </Tooltip>
      </Group>
      <Center component="main" mih="calc(100vh - 96px)">
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
