import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  apiClient,
  clearAuthTokens,
  getAccessToken,
  performTokenRefresh,
  refreshClient,
  setAuthTokens,
  setSessionCallbacks,
} from '../services/api';
import { extractApiError } from '../utils/errors';

describe('API Token Refresh Concurrency & Session Expiry Handling', () => {
  beforeEach(() => {
    clearAuthTokens();
    setSessionCallbacks({ onRefreshed: null, onExpired: null });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    clearAuthTokens();
  });

  it('stores access token only in memory and never writes to localStorage', () => {
    setAuthTokens({ accessToken: 'mem-token-1', csrfToken: 'csrf-1' });
    expect(getAccessToken()).toBe('mem-token-1');
    expect(window.localStorage.length).toBe(0);
    clearAuthTokens();
    expect(getAccessToken()).toBeNull();
  });

  it('coalesces concurrent 401 refresh requests into a single /auth/refresh call', async () => {
    const onRefreshed = vi.fn();
    setAuthTokens({ accessToken: 'expired-token', csrfToken: 'csrf-1' });
    setSessionCallbacks({ onRefreshed, onExpired: null });

    const refreshSpy = vi.spyOn(refreshClient, 'post').mockResolvedValue({
      data: {
        success: true,
        data: {
          access_token: 'fresh-token-999',
          csrf_token: 'csrf-2',
          user: { id: '1', email: 'a@b.com', roles: [] },
          permissions: ['users.read'],
        },
      },
    });

    const originalAdapter = apiClient.defaults.adapter;
    let callCount = 0;
    apiClient.defaults.adapter = async (config) => {
      callCount += 1;
      if (config.headers.Authorization === 'Bearer expired-token') {
        const error = new Error('Unauthorized');
        error.config = config;
        error.response = { status: 401, data: { error: { code: 'TOKEN_EXPIRED' } } };
        throw error;
      }
      return {
        data: { success: true, data: { ok: true, auth: config.headers.Authorization } },
        status: 200,
        statusText: 'OK',
        headers: {},
        config,
      };
    };

    try {
      const [r1, r2] = await Promise.all([apiClient.get('/users'), apiClient.get('/roles')]);
      expect(refreshSpy).toHaveBeenCalledTimes(1);
      expect(onRefreshed).toHaveBeenCalledTimes(1);
      expect(getAccessToken()).toBe('fresh-token-999');
      expect(r1.data.data.auth).toBe('Bearer fresh-token-999');
      expect(r2.data.data.auth).toBe('Bearer fresh-token-999');
      expect(callCount).toBe(4);
    } finally {
      apiClient.defaults.adapter = originalAdapter;
    }
  });

  it('invokes onExpired and clears tokens when refresh fails', async () => {
    const onExpired = vi.fn();
    setAuthTokens({ accessToken: 'stale-token', csrfToken: 'csrf-1' });
    setSessionCallbacks({ onRefreshed: null, onExpired });

    vi.spyOn(refreshClient, 'post').mockRejectedValue(new Error('Refresh rejected'));

    await expect(performTokenRefresh()).rejects.toThrow('Refresh rejected');

    expect(getAccessToken()).toBeNull();
    expect(onExpired).toHaveBeenCalledTimes(1);
  });

  it('does not trigger infinite refresh loops on /auth/login 401 errors', async () => {
    const onExpired = vi.fn();
    setSessionCallbacks({ onRefreshed: null, onExpired });

    const originalAdapter = apiClient.defaults.adapter;
    apiClient.defaults.adapter = async (config) => {
      const error = new Error('Unauthorized');
      error.config = config;
      error.response = { status: 401, data: { error: { code: 'INVALID_CREDENTIALS' } } };
      throw error;
    };

    try {
      await expect(
        apiClient.post('/auth/login', { email: 'a@b.com', password: 'x' })
      ).rejects.toThrow();
      expect(onExpired).not.toHaveBeenCalled();
    } finally {
      apiClient.defaults.adapter = originalAdapter;
    }
  });

  it('rejects non-JSON HTML SPA fallback responses as SERVICE_UNAVAILABLE', async () => {
    const originalAdapter = apiClient.defaults.adapter;
    apiClient.defaults.adapter = async (config) => ({
      data: '<!doctype html><html><body>SPA fallback</body></html>',
      status: 200,
      statusText: 'OK',
      headers: { 'content-type': 'text/html' },
      config,
    });

    try {
      await expect(apiClient.get('/users?search=john')).rejects.toMatchObject({
        code: 'SERVICE_UNAVAILABLE',
        response: { status: 503 },
      });
    } finally {
      apiClient.defaults.adapter = originalAdapter;
    }
  });

  it('maps 502/503/504 gateway errors and timeouts to clear user-facing messages', () => {
    expect(extractApiError({ response: { status: 502, data: 'Bad Gateway' } })).toMatchObject({
      code: 'SERVICE_UNAVAILABLE',
      message: 'The API server is currently unavailable. Please try again shortly.',
    });
    expect(
      extractApiError({ code: 'ECONNABORTED', message: 'timeout of 15000ms exceeded' })
    ).toMatchObject({
      code: 'REQUEST_TIMEOUT',
    });
    expect(extractApiError({ code: 'ERR_NETWORK', message: 'Network Error' })).toMatchObject({
      code: 'NETWORK_ERROR',
    });
  });
});
