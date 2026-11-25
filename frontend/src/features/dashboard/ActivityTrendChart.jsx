import { Badge, Box, Card, Center, Group, Stack, Text, Title, Tooltip } from '@mantine/core';

function formatShortUtcDate(dateStr) {
  if (!dateStr) return '';
  const parts = dateStr.split('-');
  if (parts.length !== 3) return dateStr;
  const date = new Date(Date.UTC(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2])));
  if (Number.isNaN(date.getTime())) return dateStr;
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    timeZone: 'UTC',
  }).format(date);
}

const ActivityTrendChart = ({ dailyActivity = [] }) => {
  const safeData = Array.isArray(dailyActivity) ? dailyActivity : [];
  const maxTotal = Math.max(1, ...safeData.map((d) => d.total || 0));
  const totalEvents = safeData.reduce((sum, d) => sum + (d.total || 0), 0);

  const chartHeight = 160;
  const topPad = 14;
  const bottomPad = 26;
  const plotHeight = chartHeight - topPad - bottomPad;
  const colWidth = 64;
  const barWidth = 28;
  const svgWidth = Math.max(safeData.length * colWidth, 280);

  const gridTicks = [0, Math.ceil(maxTotal / 2), maxTotal];

  return (
    <Card withBorder radius="md" p="lg" h="100%">
      <Stack gap="md" h="100%">
        <Group justify="space-between" align="flex-start" wrap="wrap" gap="xs">
          <div>
            <Title order={4}>7-Day Security &amp; Audit Activity</Title>
            <Text size="xs" c="dimmed">
              Daily UTC event volume ({totalEvents} total in last 7 days)
            </Text>
          </div>
          <Group gap="xs">
            <Badge size="xs" variant="dot" color="teal">
              Auth
            </Badge>
            <Badge size="xs" variant="dot" color="indigo">
              RBAC / Admin
            </Badge>
            <Badge size="xs" variant="dot" color="red">
              Alerts
            </Badge>
            <Badge size="xs" variant="dot" color="gray">
              Other
            </Badge>
          </Group>
        </Group>

        {safeData.length === 0 ? (
          <Center py="xl">
            <Text size="sm" c="dimmed">
              No activity data available yet.
            </Text>
          </Center>
        ) : (
          <Box style={{ width: '100%', overflowX: 'auto' }}>
            <svg
              role="img"
              aria-label={`7-day audit activity bar chart showing ${totalEvents} total events`}
              viewBox={`0 0 ${svgWidth} ${chartHeight}`}
              width="100%"
              height={chartHeight}
              style={{ display: 'block', minWidth: 280 }}
            >
              {gridTicks.map((tick, idx) => {
                const y = topPad + plotHeight - (tick / maxTotal) * plotHeight;
                return (
                  <g key={`${tick}-${idx}`}>
                    <line
                      x1={0}
                      x2={svgWidth}
                      y1={y}
                      y2={y}
                      stroke="var(--mantine-color-default-border)"
                      strokeDasharray={tick === 0 ? undefined : '3 3'}
                      strokeWidth={1}
                    />
                  </g>
                );
              })}

              {safeData.map((point, index) => {
                const total = point.total || 0;
                const authSuccess = point.auth_success || 0;
                const adminChanges = point.admin_changes || 0;
                const authFailed = point.auth_failed || 0;
                const other = Math.max(0, total - authSuccess - adminChanges - authFailed);

                const xCenter = index * colWidth + colWidth / 2;
                const xBar = xCenter - barWidth / 2;
                const baseY = topPad + plotHeight;

                const segments = [
                  { key: 'auth', value: authSuccess, fill: 'var(--mantine-color-teal-6)' },
                  { key: 'admin', value: adminChanges, fill: 'var(--mantine-color-indigo-6)' },
                  { key: 'other', value: other, fill: 'var(--mantine-color-gray-5)' },
                  { key: 'failed', value: authFailed, fill: 'var(--mantine-color-red-6)' },
                ];

                const activeSegments = segments.filter((seg) => seg.value > 0);
                const rawHeights = activeSegments.map((seg) =>
                  Math.max(2, (seg.value / maxTotal) * plotHeight)
                );
                const rawSum = rawHeights.reduce((acc, h) => acc + h, 0);
                const scale = rawSum > plotHeight ? plotHeight / rawSum : 1;

                let currentY = baseY;
                const tooltipText = `${formatShortUtcDate(point.date)}: ${total} total (${authSuccess} auth, ${adminChanges} admin, ${authFailed} alerts, ${other} other)`;

                return (
                  <Tooltip key={point.date} label={tooltipText} withArrow position="top">
                    <g
                      tabIndex={0}
                      role="img"
                      aria-label={tooltipText}
                      style={{ cursor: 'pointer' }}
                    >
                      <title>{tooltipText}</title>
                      <rect
                        x={index * colWidth + 4}
                        y={topPad}
                        width={colWidth - 8}
                        height={plotHeight}
                        fill="transparent"
                      />
                      {total === 0 ? (
                        <rect
                          x={xBar}
                          y={baseY - 2}
                          width={barWidth}
                          height={2}
                          rx={1}
                          fill="var(--mantine-color-default-border)"
                        />
                      ) : (
                        activeSegments.map((seg, segIdx) => {
                          const segHeight = rawHeights[segIdx] * scale;
                          currentY -= segHeight;
                          return (
                            <rect
                              key={seg.key}
                              x={xBar}
                              y={currentY}
                              width={barWidth}
                              height={segHeight}
                              rx={2}
                              fill={seg.fill}
                            />
                          );
                        })
                      )}
                      {total > 0 && (
                        <text
                          x={xCenter}
                          y={Math.max(10, currentY - 4)}
                          textAnchor="middle"
                          fontSize="10"
                          fontWeight="600"
                          fill="var(--mantine-color-text)"
                        >
                          {total}
                        </text>
                      )}
                      <text
                        x={xCenter}
                        y={chartHeight - 6}
                        textAnchor="middle"
                        fontSize="11"
                        fill="var(--mantine-color-dimmed)"
                      >
                        {formatShortUtcDate(point.date)}
                      </text>
                    </g>
                  </Tooltip>
                );
              })}
            </svg>
          </Box>
        )}
      </Stack>
    </Card>
  );
};

export default ActivityTrendChart;
