import { MantineProvider } from '@mantine/core';
import { ModalsProvider } from '@mantine/modals';
import { Notifications } from '@mantine/notifications';
import { render } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import AuthProvider from '../contexts/AuthContext';
import PermissionProvider from '../contexts/PermissionContext';
import { theme } from '../theme';

export function renderWithProviders(
  ui,
  { initialEntries = ['/'], initialSession = null, skipRestore = true } = {}
) {
  return render(
    <MantineProvider theme={theme}>
      <Notifications />
      <ModalsProvider>
        <MemoryRouter initialEntries={initialEntries}>
          <AuthProvider initialSession={initialSession} skipRestore={skipRestore}>
            <PermissionProvider>{ui}</PermissionProvider>
          </AuthProvider>
        </MemoryRouter>
      </ModalsProvider>
    </MantineProvider>
  );
}
