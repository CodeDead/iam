export function extractApiError(error, fallback = 'An unexpected error occurred') {
  if (!error) {
    return { code: 'UNKNOWN_ERROR', message: fallback, details: null, status: null };
  }

  const status = error.response?.status ?? null;
  const payload = error.response?.data?.error;

  if (payload) {
    return {
      code: payload.code || 'API_ERROR',
      message: payload.message || fallback,
      details: payload.details || null,
      status,
    };
  }

  if (error.message === 'Network Error') {
    return {
      code: 'NETWORK_ERROR',
      message: 'Unable to connect to the server. Please verify the API is running.',
      details: null,
      status,
    };
  }

  return {
    code: 'REQUEST_FAILED',
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
