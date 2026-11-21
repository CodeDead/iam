export function formatDateTime(isoString) {
  if (!isoString) {
    return 'Never';
  }
  const date = new Date(isoString);
  if (Number.isNaN(date.getTime())) {
    return 'Invalid date';
  }
  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}

export function formatUserName(user) {
  if (!user) return '';
  return `${user.first_name || ''} ${user.last_name || ''}`.trim();
}

export function groupPermissionsByResource(permissions) {
  const groups = {};
  for (const perm of permissions || []) {
    const key = perm.resource || 'other';
    if (!groups[key]) {
      groups[key] = [];
    }
    groups[key].push(perm);
  }
  return Object.entries(groups)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([resource, items]) => ({
      resource,
      items: items.sort((a, b) => a.action.localeCompare(b.action)),
    }));
}

export function capitalize(str) {
  if (!str) return '';
  return str.charAt(0).toUpperCase() + str.slice(1).replace(/_/g, ' ');
}
