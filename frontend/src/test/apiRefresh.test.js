import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  apiClient,
  clearAuthTokens,
  getAccessToken,
  performTokenRefresh,
  refreshClient,
  resetCrossTabSyncState,
  setAuthTokens,
  setSessionCallbacks,
} from '../services/api';
import { extractApiError } from '../utils/errors';

describe('API Token Refresh Concurrency & Session Expiry Handling', () => {
  beforeEach(() => {
    resetCrossTabSyncState();
    clearAuthTokens();
    setSessionCallbacks({ onRefreshed: null, onExpired: null });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    resetCrossTabSyncState();
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
    expect(
      extractApiError({
        response: {
          status: 409,
          data: {
            code: 'LAST_SUPER_ADMIN_PROTECTED',
            message: 'Cannot delete the last active Super Administrator account',
          },
        },
      })
    ).toMatchObject({
      code: 'LAST_SUPER_ADMIN_PROTECTED',
      message: 'Cannot delete the last active Super Administrator account',
      status: 409,
    });
    expect(
      extractApiError({
        response: {
          status: 409,
          data: JSON.stringify({
            success: false,
            error: {
              code: 'LAST_SUPER_ADMIN_PROTECTED',
              message: 'Cannot delete the last active Super Administrator account',
            },
          }),
        },
      })
    ).toMatchObject({
      code: 'LAST_SUPER_ADMIN_PROTECTED',
      message: 'Cannot delete the last active Super Administrator account',
      status: 409,
    });
  });

  it('reuses a cross-tab broadcast refresh when navigator.locks reports contention', async () => {
    const channels = [];
    class FakeBroadcastChannel {
      constructor(name) {
        this.name = name;
        this.onmessage = null;
        channels.push(this);
      }
      postMessage(data) {
        for (const ch of channels) {
          if (ch !== this && ch.name === this.name && typeof ch.onmessage === 'function') {
            ch.onmessage({ data });
          }
        }
      }
      close() {}
    }

    const originalBC = globalThis.BroadcastChannel;
    const originalLocks = navigator.locks;
    globalThis.BroadcastChannel = FakeBroadcastChannel;
    resetCrossTabSyncState();

    const syncedPayload = {
      access_token: 'synced-from-tab-1',
      csrf_token: 'csrf-from-tab-1',
      user: { id: 'u1', email: 'admin@example.com', roles: [] },
      permissions: ['users.read'],
    };

    Object.defineProperty(navigator, 'locks', {
      configurable: true,
      value: {
        request: vi.fn(async (_name, optionsOrCb, maybeCb) => {
          if (typeof optionsOrCb === 'object' && optionsOrCb?.ifAvailable) {
            // Simulate another tab holding the lock right now
            return maybeCb(null);
          }
          const cb = typeof optionsOrCb === 'function' ? optionsOrCb : maybeCb;
          // Simulate Tab 1 broadcasting its fresh session right as it releases the lock
          const tab1Channel = new FakeBroadcastChannel('codedead-iam-auth-sync');
          tab1Channel.postMessage({ type: 'AUTH_REFRESHED', payload: syncedPayload });
          return cb({});
        }),
      },
    });

    const refreshSpy = vi.spyOn(refreshClient, 'post');
    const onRefreshed = vi.fn();
    setAuthTokens({ accessToken: 'old-token', csrfToken: 'old-csrf' });
    setSessionCallbacks({ onRefreshed, onExpired: null });

    try {
      const result = await performTokenRefresh();
      expect(result.access_token).toBe('synced-from-tab-1');
      expect(getAccessToken()).toBe('synced-from-tab-1');
      expect(refreshSpy).not.toHaveBeenCalled();
      expect(onRefreshed).toHaveBeenCalledWith(syncedPayload);
    } finally {
      globalThis.BroadcastChannel = originalBC;
      Object.defineProperty(navigator, 'locks', { configurable: true, value: originalLocks });
    }
  });

  it('clears tokens and triggers onExpired when another tab broadcasts AUTH_LOGGED_OUT', () => {
    const channels = [];
    class FakeBroadcastChannel {
      constructor(name) {
        this.name = name;
        this.onmessage = null;
        channels.push(this);
      }
      postMessage(data) {
        for (const ch of channels) {
          if (ch !== this && ch.name === this.name && typeof ch.onmessage === 'function') {
            ch.onmessage({ data });
          }
        }
      }
      close() {}
    }

    const originalBC = globalThis.BroadcastChannel;
    globalThis.BroadcastChannel = FakeBroadcastChannel;
    resetCrossTabSyncState();
    const onExpired = vi.fn();

    try {
      setAuthTokens({ accessToken: 'active-token', csrfToken: 'csrf-1' });
      setSessionCallbacks({ onRefreshed: null, onExpired });

      const otherTab = new FakeBroadcastChannel('codedead-iam-auth-sync');
      otherTab.postMessage({ type: 'AUTH_LOGGED_OUT' });

      expect(getAccessToken()).toBeNull();
      expect(onExpired).toHaveBeenCalledTimes(1);
    } finally {
      globalThis.BroadcastChannel = originalBC;
    }
  });
});
