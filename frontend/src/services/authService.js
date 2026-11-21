import { apiClient, clearAuthTokens, performTokenRefresh, setAuthTokens } from './api';

export const authService = {
  async register(payload) {
    const response = await apiClient.post('/auth/register', payload);
    const data = response.data.data;
    setAuthTokens({ accessToken: data.access_token, csrfToken: data.csrf_token });
    return data;
  },

  async login(payload) {
    const response = await apiClient.post('/auth/login', payload);
    const data = response.data.data;
    setAuthTokens({ accessToken: data.access_token, csrfToken: data.csrf_token });
    return data;
  },

  async refresh() {
    return performTokenRefresh();
  },

  async logout() {
    try {
      await apiClient.post('/auth/logout');
    } finally {
      clearAuthTokens();
    }
  },

  async me() {
    const response = await apiClient.get('/auth/me');
    return response.data.data;
  },
};
