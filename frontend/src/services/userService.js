import { apiClient } from './api';

export const userService = {
  async list(params = {}) {
    const cleanParams = Object.fromEntries(
      Object.entries(params).filter(([, v]) => v !== '' && v !== null && v !== undefined)
    );
    const response = await apiClient.get('/users', { params: cleanParams });
    return response.data.data;
  },

  async getById(id) {
    const response = await apiClient.get(`/users/${id}`);
    return response.data.data;
  },

  async create(payload) {
    const response = await apiClient.post('/users', payload);
    return response.data.data;
  },

  async update(id, payload) {
    const response = await apiClient.put(`/users/${id}`, payload);
    return response.data.data;
  },

  async updateStatus(id, status) {
    const response = await apiClient.patch(`/users/${id}/status`, { status });
    return response.data.data;
  },

  async remove(id) {
    await apiClient.delete(`/users/${id}`);
  },

  async getRoles(id) {
    const response = await apiClient.get(`/users/${id}/roles`);
    return response.data.data;
  },

  async updateRoles(id, roleIds) {
    const response = await apiClient.put(`/users/${id}/roles`, { role_ids: roleIds });
    return response.data.data;
  },

  async getEffectivePermissions(id) {
    const response = await apiClient.get(`/users/${id}/permissions`);
    return response.data.data;
  },

  async getOrganisations(id) {
    const response = await apiClient.get(`/users/${id}/organisations`);
    return response.data.data;
  },
};
