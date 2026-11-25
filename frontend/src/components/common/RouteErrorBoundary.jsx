import { Component } from 'react';
import { Button, Center, Group, Paper, Stack, Text, ThemeIcon, Title } from '@mantine/core';
import { IconAlertTriangle, IconRefresh } from '@tabler/icons-react';

class RouteErrorBoundary extends Component {
  constructor(props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidUpdate(prevProps) {
    if (this.state.hasError && prevProps.resetKey !== this.props.resetKey) {
      this.setState({ hasError: false });
    }
  }

  handleRetry = () => {
    this.setState({ hasError: false });
  };

  handleReload = () => {
    window.location.reload();
  };

  render() {
    if (this.state.hasError) {
      return (
        <Center mih="60vh" p="md" role="alert">
          <Paper withBorder p="xl" radius="md" maw={440} w="100%">
            <Stack align="center" ta="center" gap="md">
              <ThemeIcon size={56} radius="xl" color="red" variant="light">
                <IconAlertTriangle size={28} />
              </ThemeIcon>
              <Title order={3}>Unable to load this page</Title>
              <Text size="sm" c="dimmed">
                A page module failed to load, which can happen after an application update or a
                temporary network interruption.
              </Text>
              <Group gap="sm">
                <Button variant="default" onClick={this.handleRetry}>
                  Try again
                </Button>
                <Button leftSection={<IconRefresh size={16} />} onClick={this.handleReload}>
                  Reload page
                </Button>
              </Group>
            </Stack>
          </Paper>
        </Center>
      );
    }

    return this.props.children;
  }
}

export default RouteErrorBoundary;
