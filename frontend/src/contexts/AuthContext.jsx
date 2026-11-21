import { useCallback, useEffect, useMemo, useState } from 'react';
import { authService } from '../services/authService';
import { clearAuthTokens, setSessionCallbacks } from '../services/api';
import { AuthContext } from './contextInstances';

export function AuthProvider({ children, initialSession = null, skipRestore = false }) {
  const [user, setUser] = useState(initialSession?.user || null);
  const [permissions, setPermissions] = useState(initialSession?.permissions || []);
  const [isLoading, setIsLoading] = useState(!initialSession && !skipRestore);
  const [wasLoggedOut, setWasLoggedOut] = useState(false);

  const applySession = useCallback((sessionData) => {
    setWasLoggedOut(false);
    setUser(sessionData?.user || null);
    setPermissions(sessionData?.permissions || []);
  }, []);

  const clearSession = useCallback(() => {
    clearAuthTokens();
    setUser(null);
    setPermissions([]);
  }, []);

  useEffect(() => {
    setSessionCallbacks({
      onRefreshed: (data) => applySession(data),
      onExpired: () => clearSession(),
    });
    return () => {
      setSessionCallbacks({ onRefreshed: null, onExpired: null });
    };
  }, [applySession, clearSession]);

  useEffect(() => {
    if (initialSession || skipRestore) {
      return;
    }

    let cancelled = false;
    (async () => {
      try {
        const refreshed = await authService.refresh();
        if (!cancelled) {
          applySession(refreshed);
        }
      } catch {
        if (!cancelled) {
          clearSession();
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [applySession, clearSession, initialSession, skipRestore]);

  const login = useCallback(
    async (credentials) => {
      const data = await authService.login(credentials);
      applySession(data);
      return data;
    },
    [applySession]
  );

  const register = useCallback(
    async (payload) => {
      const data = await authService.register(payload);
      applySession(data);
      return data;
    },
    [applySession]
  );

  const logout = useCallback(async () => {
    try {
      await authService.logout();
    } finally {
      setWasLoggedOut(true);
      clearSession();
    }
  }, [clearSession]);

  const refreshProfile = useCallback(async () => {
    const data = await authService.me();
    applySession(data);
    return data;
  }, [applySession]);

  const value = useMemo(
    () => ({
      user,
      permissions,
      isAuthenticated: Boolean(user),
      isLoading,
      wasLoggedOut,
      login,
      register,
      logout,
      refreshProfile,
      updateUser: setUser,
    }),
    [user, permissions, isLoading, wasLoggedOut, login, register, logout, refreshProfile]
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
