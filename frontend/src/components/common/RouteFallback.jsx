import { Center, Loader } from '@mantine/core';

const RouteFallback = ({ minHeight = '60vh', label = 'Loading page' }) => {
  return (
    <Center mih={minHeight} role="status" aria-live="polite" aria-label={label}>
      <Loader size="md" />
    </Center>
  );
};

export default RouteFallback;
