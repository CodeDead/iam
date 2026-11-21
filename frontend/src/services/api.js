import axios from 'axios';
import { API_BASE_URL, CSRF_COOKIE_NAME, CSRF_HEADER_NAME } from '../config/api';
import { getCookie } from '../utils/cookies';

// Access token and CSRF token are kept strictly in memory (never localStorage).
let accessToken = null;
let csrfTokenMemory = null;
let refreshPromise = null;

let sessionCallbacks = {
  onRefreshed: null,
  onExpired: null,
};

export function setSessionCallbacks({ onRefreshed, onExpired }) {
  sessionCallbacks = { onRefreshed, onExpired };
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

export async function performTokenRefresh() {
  if (refreshPromise) {
    return refreshPromise;
  }

  refreshPromise = (async () => {
    try {
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

      return data;
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
