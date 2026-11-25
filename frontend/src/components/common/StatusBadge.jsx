import { Badge } from '@mantine/core';

const STATUS_META = {
  active: { color: 'green', label: 'Active' },
  disabled: { color: 'red', label: 'Disabled' },
  locked: { color: 'orange', label: 'Locked' },
  pending_verification: { color: 'yellow', label: 'Pending Verification' },
};

const StatusBadge = ({ status }) => {
  const meta = STATUS_META[status] || { color: 'gray', label: status || 'Unknown' };
  return (
    <Badge color={meta.color} variant="light" radius="sm">
      {meta.label}
    </Badge>
  );
};

export default StatusBadge;
