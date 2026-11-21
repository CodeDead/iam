import { apiClient } from './api';

export const permissionService = {
  async list(params = {}) {
    const cleanParams = Object.fromEntries(
      Object.entries(params).filter(([, v]) => v !== '' && v !== null && v !== undefined)
    );
    const response = await apiClient.get('/permissions', { params: cleanParams });
    return response.data.data;
  },

  async getById(id) {
    const response = await apiClient.get(`/permissions/${id}`);
    return response.data.data;
  },

  async create(payload) {
    const response = await apiClient.post('/permissions', payload);
    return response.data.data;
  },

  async update(id, payload) {
    const response = await apiClient.put(`/permissions/${id}`, payload);
    return response.data.data;
  },

  async remove(id) {
    await apiClient.delete(`/permissions/${id}`);
  },

  async getAssociatedRoles(id) {
    const response = await apiClient.get(`/permissions/${id}/roles`);
    return response.data.data;
  },
};
