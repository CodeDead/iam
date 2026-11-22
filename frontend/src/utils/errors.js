function parseResponseData(rawData) {
  if (typeof rawData !== 'string') {
    return rawData;
  }
  const trimmed = rawData.trim();
  if (!trimmed.startsWith('{')) {
    return rawData;
  }
  try {
    return JSON.parse(trimmed);
  } catch {
    return rawData;
  }
}

function extractStructuredPayload(data) {
  if (!data || typeof data !== 'object') {
    return null;
  }
  if (data.error && typeof data.error === 'object') {
    return data.error;
  }
  if (typeof data.message === 'string' && data.message.trim().length > 0) {
    return data;
  }
  return null;
}

export function extractApiError(error, fallback = 'An unexpected error occurred') {
  if (!error) {
    return { code: 'UNKNOWN_ERROR', message: fallback, details: null, status: null };
  }

  const status = error.response?.status ?? null;
  const responseData = parseResponseData(error.response?.data);
  const payload = extractStructuredPayload(responseData);

  if (payload) {
    return {
      code: payload.code || 'API_ERROR',
      message: payload.message || fallback,
      details: payload.details || null,
      status,
    };
  }

  if (error.code === 'ECONNABORTED' || error.message?.toLowerCase().includes('timeout')) {
    return {
      code: 'REQUEST_TIMEOUT',
      message: 'The request timed out. The API server may be offline or overloaded.',
      details: null,
      status,
    };
  }

  if (status === 502 || status === 503 || status === 504) {
    return {
      code: 'SERVICE_UNAVAILABLE',
      message: 'The API server is currently unavailable. Please try again shortly.',
      details: null,
      status,
    };
  }

  if (
    error.code === 'ERR_NETWORK' ||
    error.message === 'Network Error' ||
    (!error.response && error.request)
  ) {
    return {
      code: 'NETWORK_ERROR',
      message: 'Unable to connect to the server. Please verify the API is running.',
      details: null,
      status,
    };
  }

  return {
    code: error.code || 'REQUEST_FAILED',
    message: error.message || fallback,
    details: null,
    status,
  };
}

export function formatValidationDetails(details) {
  if (!details || typeof details !== 'object') {
    return [];
  }
  return Object.entries(details).flatMap(([field, messages]) =>
    Array.isArray(messages) ? messages.map((m) => `${field}: ${m}`) : [`${field}: ${messages}`]
  );
}
