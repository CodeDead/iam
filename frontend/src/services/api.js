import axios from 'axios';
import { API_BASE_URL, CSRF_COOKIE_NAME, CSRF_HEADER_NAME } from '../config/api';
import { getCookie } from '../utils/cookies';

// Access token and CSRF token are kept strictly in memory (never localStorage).
let accessToken = null;
let csrfTokenMemory = null;
let refreshPromise = null;

// Cross-tab auth coordination state (in-memory only).
const AUTH_REFRESH_LOCK_NAME = 'codedead-iam-auth-refresh';
const AUTH_BROADCAST_CHANNEL_NAME = 'codedead-iam-auth-sync';
let authBroadcastChannel = null;
let lastSyncedSession = null;
let lastSyncedAt = 0;
const syncWaiters = new Set();

let sessionCallbacks = {
  onRefreshed: null,
  onExpired: null,
};

function notifySyncWaiters(event) {
  for (const waiter of Array.from(syncWaiters)) {
    syncWaiters.delete(waiter);
    waiter(event);
  }
}

function handleBroadcastMessage(event) {
  const message = event?.data;
  if (!message || typeof message !== 'object') {
    return;
  }

  if (message.type === 'AUTH_REFRESHED' && message.payload?.access_token) {
    lastSyncedSession = message.payload;
    lastSyncedAt = Date.now();
    setAuthTokens({
      accessToken: message.payload.access_token,
      csrfToken: message.payload.csrf_token,
    });
    if (sessionCallbacks.onRefreshed) {
      sessionCallbacks.onRefreshed(message.payload);
    }
    notifySyncWaiters({ type: 'refreshed', data: message.payload });
  } else if (message.type === 'AUTH_LOGGED_OUT') {
    lastSyncedSession = null;
    lastSyncedAt = 0;
    clearAuthTokens();
    if (sessionCallbacks.onExpired) {
      sessionCallbacks.onExpired();
    }
    notifySyncWaiters({ type: 'logged_out' });
  }
}

function ensureBroadcastChannel() {
  if (authBroadcastChannel) {
    return authBroadcastChannel;
  }
  if (typeof BroadcastChannel === 'undefined') {
    return null;
  }
  try {
    authBroadcastChannel = new BroadcastChannel(AUTH_BROADCAST_CHANNEL_NAME);
    authBroadcastChannel.onmessage = handleBroadcastMessage;
    return authBroadcastChannel;
  } catch {
    return null;
  }
}

export function broadcastAuthRefreshed(data) {
  const channel = ensureBroadcastChannel();
  if (!channel || !data?.access_token) {
    return;
  }
  try {
    channel.postMessage({ type: 'AUTH_REFRESHED', payload: data });
  } catch {
    // Ignore closed/unsupported channel errors.
  }
}

export function broadcastAuthLoggedOut() {
  lastSyncedSession = null;
  lastSyncedAt = 0;
  const channel = ensureBroadcastChannel();
  if (!channel) {
    return;
  }
  try {
    channel.postMessage({ type: 'AUTH_LOGGED_OUT' });
  } catch {
    // Ignore closed/unsupported channel errors.
  }
}

// Initialize cross-tab listener in browser environments so logout & refresh sync across tabs.
ensureBroadcastChannel();

export function setSessionCallbacks({ onRefreshed, onExpired }) {
  sessionCallbacks = { onRefreshed, onExpired };
  ensureBroadcastChannel();
}

export function setAuthTokens({ accessToken: nextAccess, csrfToken: nextCsrf }) {
  accessToken = nextAccess || null;
  if (nextCsrf !== undefined) {
    csrfTokenMemory = nextCsrf || null;
  }
}

export function getAccessToken() {
  return accessToken;
}

export function getCsrfToken() {
  return getCookie(CSRF_COOKIE_NAME) || csrfTokenMemory;
}

export function clearAuthTokens() {
  accessToken = null;
  csrfTokenMemory = null;
}

export function resetCrossTabSyncState() {
  lastSyncedSession = null;
  lastSyncedAt = 0;
  syncWaiters.clear();
  if (authBroadcastChannel) {
    try {
      authBroadcastChannel.close();
    } catch {
      // Ignore close errors.
    }
    authBroadcastChannel = null;
  }
}

export const apiClient = axios.create({
  baseURL: API_BASE_URL,
  withCredentials: true,
  timeout: 15000,
  headers: {
    'Content-Type': 'application/json',
    Accept: 'application/json',
  },
});

export const refreshClient = axios.create({
  baseURL: API_BASE_URL,
  withCredentials: true,
  timeout: 15000,
  headers: {
    'Content-Type': 'application/json',
    Accept: 'application/json',
  },
});

const AUTH_NO_REFRESH_PATHS = ['/auth/login', '/auth/register', '/auth/refresh', '/auth/logout'];

function shouldSkipRefresh(url = '') {
  return AUTH_NO_REFRESH_PATHS.some((path) => url.includes(path));
}

async function executeHttpRefresh() {
  const csrf = getCsrfToken();
  const headers = csrf ? { [CSRF_HEADER_NAME]: csrf } : {};
  const response = await refreshClient.post('/auth/refresh', {}, { headers });
  const data = response.data?.data;

  if (!data?.access_token) {
    throw new Error('Refresh response missing access token');
  }

  setAuthTokens({
    accessToken: data.access_token,
    csrfToken: data.csrf_token,
  });

  if (sessionCallbacks.onRefreshed) {
    sessionCallbacks.onRefreshed(data);
  }

  broadcastAuthRefreshed(data);
  return data;
}

function waitForBroadcastSync(timeoutMs = 50) {
  return new Promise((resolve) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        syncWaiters.delete(onEvent);
        resolve(null);
      }
    }, timeoutMs);

    function onEvent(event) {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        resolve(event);
      }
    }

    syncWaiters.add(onEvent);
  });
}

async function runCoordinatedRefresh() {
  ensureBroadcastChannel();
  const webLocks =
    typeof navigator !== 'undefined' && typeof navigator.locks?.request === 'function'
      ? navigator.locks
      : null;

  if (!webLocks) {
    return executeHttpRefresh();
  }

  const startedAt = Date.now();
  const initialAccessToken = accessToken;

  // Fast path: if no other tab holds the refresh lock, execute immediately without delay.
  const immediateResult = await webLocks.request(
    AUTH_REFRESH_LOCK_NAME,
    { ifAvailable: true },
    async (lock) => {
      if (!lock) {
        return { contended: true };
      }
      const data = await executeHttpRefresh();
      return { contended: false, data };
    }
  );

  if (!immediateResult.contended) {
    return immediateResult.data;
  }

  // Another tab is actively rotating the refresh cookie. Wait for the lock to release
  // and reuse the freshly broadcast access token if one arrives, avoiding duplicate /refresh calls.
  return webLocks.request(AUTH_REFRESH_LOCK_NAME, async () => {
    if (
      lastSyncedSession?.access_token &&
      lastSyncedAt >= startedAt &&
      lastSyncedSession.access_token !== initialAccessToken
    ) {
      return lastSyncedSession;
    }

    const syncedEvent = await waitForBroadcastSync(50);
    if (syncedEvent?.type === 'refreshed' && syncedEvent.data?.access_token) {
      return syncedEvent.data;
    }
    if (syncedEvent?.type === 'logged_out') {
      throw new Error('Session signed out in another tab');
    }

    if (
      lastSyncedSession?.access_token &&
      lastSyncedAt >= startedAt &&
      lastSyncedSession.access_token !== initialAccessToken
    ) {
      return lastSyncedSession;
    }

    // Fallback: cookies have now been updated by the previous tab, so refreshing uses the new token.
    return executeHttpRefresh();
  });
}

export async function performTokenRefresh() {
  if (refreshPromise) {
    return refreshPromise;
  }

  refreshPromise = (async () => {
    try {
      return await runCoordinatedRefresh();
    } catch (err) {
      clearAuthTokens();
      if (sessionCallbacks.onExpired) {
        sessionCallbacks.onExpired();
      }
      throw err;
    } finally {
      refreshPromise = null;
    }
  })();

  return refreshPromise;
}

apiClient.interceptors.request.use((config) => {
  const nextConfig = { ...config, headers: { ...(config.headers || {}) } };
  if (accessToken) {
    nextConfig.headers.Authorization = `Bearer ${accessToken}`;
  }
  const method = (nextConfig.method || 'get').toUpperCase();
  if (method !== 'GET' && method !== 'HEAD' && method !== 'OPTIONS') {
    const csrf = getCsrfToken();
    if (csrf) {
      nextConfig.headers[CSRF_HEADER_NAME] = csrf;
    }
  }
  return nextConfig;
});

function assertJsonApiResponse(response) {
  if (response.status === 204) {
    return response;
  }

  const contentType = String(
    response.headers?.['content-type'] || response.headers?.['Content-Type'] || ''
  ).toLowerCase();
  const isApiEnvelope =
    typeof response.data === 'object' &&
    response.data !== null &&
    Object.prototype.hasOwnProperty.call(response.data, 'success');

  if (!isApiEnvelope || (contentType && !contentType.includes('application/json'))) {
    const err = new Error('The API server is currently unavailable. Please try again shortly.');
    err.code = 'SERVICE_UNAVAILABLE';
    err.config = response.config;
    err.response = {
      status: 503,
      data: {
        success: false,
        error: {
          code: 'SERVICE_UNAVAILABLE',
          message: 'The API server is currently unavailable. Please try again shortly.',
        },
      },
    };
    return Promise.reject(err);
  }

  return response;
}

apiClient.interceptors.response.use(
  (response) => assertJsonApiResponse(response),
  async (error) => {
    const originalRequest = error.config;
    const status = error.response?.status;

    if (
      status !== 401 ||
      !originalRequest ||
      originalRequest._retry ||
      shouldSkipRefresh(originalRequest.url)
    ) {
      return Promise.reject(error);
    }

    originalRequest._retry = true;

    try {
      const data = await performTokenRefresh();
      originalRequest.headers = {
        ...(originalRequest.headers || {}),
        Authorization: `Bearer ${data.access_token}`,
      };
      return apiClient(originalRequest);
    } catch (refreshError) {
      return Promise.reject(refreshError);
    }
  }
);
