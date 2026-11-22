export function getCookie(name) {
  if (typeof document === 'undefined' || !document.cookie) {
    return null;
  }
  const cookies = document.cookie.split(';');
  for (const raw of cookies) {
    const [k, ...rest] = raw.trim().split('=');
    if (k === name) {
      const value = rest.join('=');
      try {
        return decodeURIComponent(value);
      } catch {
        return value;
      }
    }
  }
  return null;
}
