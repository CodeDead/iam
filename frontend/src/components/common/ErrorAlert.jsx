import { Alert, List, Text } from '@mantine/core';
import { IconAlertCircle } from '@tabler/icons-react';
import { formatValidationDetails } from '../../utils/errors';

const ErrorAlert = ({ error, title = 'Error', onClose }) => {
  if (!error) return null;

  const message = typeof error === 'string' ? error : error.message;
  const details = typeof error === 'object' ? formatValidationDetails(error.details) : [];

  return (
    <Alert
      icon={<IconAlertCircle size={18} />}
      title={title}
      color="red"
      variant="light"
      withCloseButton={Boolean(onClose)}
      onClose={onClose}
      mb="md"
    >
      <Text size="sm">{message}</Text>
      {details.length > 0 && (
        <List size="xs" mt="xs" spacing={2}>
          {details.map((line, idx) => (
            <List.Item key={idx}>{line}</List.Item>
          ))}
        </List>
      )}
    </Alert>
  );
};

export default ErrorAlert;
