import '@mantine/core/styles.css';
import '@mantine/notifications/styles.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';

const CHUNK_RELOAD_KEY = 'codedead_iam_chunk_reload_at';
const CHUNK_RELOAD_COOLDOWN_MS = 10_000;

window.addEventListener('vite:preloadError', (event) => {
  try {
    const lastAttempt = Number(window.sessionStorage.getItem(CHUNK_RELOAD_KEY) || 0);
    if (Date.now() - lastAttempt > CHUNK_RELOAD_COOLDOWN_MS) {
      window.sessionStorage.setItem(CHUNK_RELOAD_KEY, String(Date.now()));
      event.preventDefault();
      window.location.reload();
    }
  } catch {
    // Let RouteErrorBoundary display a manual reload prompt if storage is unavailable.
  }
});

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <App />
  </StrictMode>
);
