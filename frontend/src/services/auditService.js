import { apiClient } from './api';

export const auditService = {
  async list(params = {}) {
    const cleanParams = Object.fromEntries(
      Object.entries(params).filter(([, v]) => v !== '' && v !== null && v !== undefined)
    );
    const response = await apiClient.get('/audit-logs', { params: cleanParams });
    return response.data.data;
  },

  async getById(id) {
    const response = await apiClient.get(`/audit-logs/${id}`);
    return response.data.data;
  },
};
