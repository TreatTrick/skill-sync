import { t } from '@/shared/i18n'

import type { DemoMetric } from '../types/dashboardData'

interface MetricCardProps {
  metric: DemoMetric
}

export const MetricCard = ({ metric }: MetricCardProps) => (
  <div className="rounded-lg border border-border bg-surface p-4">
    <div className="text-sm font-medium text-muted-foreground">
      {t(metric.labelKey)}
    </div>
    <div className="mt-3 flex items-end justify-between gap-3">
      <div className="text-2xl font-bold text-strong-foreground">
        {metric.value}
      </div>
      <div className="rounded-full bg-success-muted px-2 py-1 text-xs font-bold text-success">
        {metric.detail}
      </div>
    </div>
  </div>
)
