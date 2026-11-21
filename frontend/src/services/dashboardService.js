import { apiClient } from './api';

export const dashboardService = {
  async getStats() {
    const response = await apiClient.get('/dashboard/stats');
    return response.data.data;
  },
};
