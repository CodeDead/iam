import { apiClient } from './api';

export const roleService = {
  async list(params = {}) {
    const cleanParams = Object.fromEntries(
      Object.entries(params).filter(([, v]) => v !== '' && v !== null && v !== undefined)
    );
    const response = await apiClient.get('/roles', { params: cleanParams });
    return response.data.data;
  },

  async getById(id) {
    const response = await apiClient.get(`/roles/${id}`);
    return response.data.data;
  },

  async create(payload) {
    const response = await apiClient.post('/roles', payload);
    return response.data.data;
  },

  async update(id, payload) {
    const response = await apiClient.put(`/roles/${id}`, payload);
    return response.data.data;
  },

  async remove(id) {
    await apiClient.delete(`/roles/${id}`);
  },

  async getPermissions(id) {
    const response = await apiClient.get(`/roles/${id}/permissions`);
    return response.data.data;
  },

  async updatePermissions(id, permissionIds) {
    const response = await apiClient.put(`/roles/${id}/permissions`, {
      permission_ids: permissionIds,
    });
    return response.data.data;
  },

  async getAssignedUsers(id) {
    const response = await apiClient.get(`/roles/${id}/users`);
    return response.data.data;
  },
};
