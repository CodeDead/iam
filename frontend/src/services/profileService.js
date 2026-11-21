import { apiClient } from './api';

export const profileService = {
  async getProfile() {
    const response = await apiClient.get('/profile');
    return response.data.data;
  },

  async updateProfile(payload) {
    const response = await apiClient.put('/profile', payload);
    return response.data.data;
  },

  async changePassword(payload) {
    const response = await apiClient.patch('/profile/password', payload);
    return response.data;
  },
};
