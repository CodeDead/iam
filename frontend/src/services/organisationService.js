import { apiClient } from './api';

export const organisationService = {
  async listMine() {
    const response = await apiClient.get('/organisations/me');
    return response.data.data;
  },

  async list(params = {}) {
    const cleanParams = Object.fromEntries(
      Object.entries(params).filter(([, v]) => v !== '' && v !== null && v !== undefined)
    );
    const response = await apiClient.get('/organisations', { params: cleanParams });
    return response.data.data;
  },

  async getById(id) {
    const response = await apiClient.get(`/organisations/${id}`);
    return response.data.data;
  },

  async create(payload) {
    const response = await apiClient.post('/organisations', payload);
    return response.data.data;
  },

  async update(id, payload) {
    const response = await apiClient.put(`/organisations/${id}`, payload);
    return response.data.data;
  },

  async remove(id) {
    await apiClient.delete(`/organisations/${id}`);
  },

  async addMember(id, payload) {
    const response = await apiClient.post(`/organisations/${id}/members`, payload);
    return response.data.data;
  },

  async updateMember(id, userId, payload) {
    const response = await apiClient.patch(`/organisations/${id}/members/${userId}`, payload);
    return response.data.data;
  },

  async removeMember(id, userId) {
    const response = await apiClient.delete(`/organisations/${id}/members/${userId}`);
    return response.data.data;
  },

  async acceptInvitation(id) {
    const response = await apiClient.post(`/organisations/${id}/accept`);
    return response.data.data;
  },

  async leave(id) {
    await apiClient.post(`/organisations/${id}/leave`);
  },
};
