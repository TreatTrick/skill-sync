import { Badge } from '@/shared/ui'
import { t } from '@/shared/i18n'

import type { DemoActivity } from '../types/dashboardData'

interface ActivityTableProps {
  activities: DemoActivity[]
}

const getStatusVariant = (status: DemoActivity['status']) =>
  status === 'done' ? 'success' : 'warning'

export const ActivityTable = ({ activities }: ActivityTableProps) => (
  <div className="overflow-x-auto rounded-lg border border-border bg-surface">
    <table className="min-w-[720px] w-full border-collapse text-left text-sm">
      <thead className="bg-surface-muted text-xs font-bold text-muted-foreground">
        <tr>
          <th className="px-4 py-3">{t('demoDashboard.activity.title')}</th>
          <th className="px-4 py-3">{t('demoDashboard.activity.owner')}</th>
          <th className="px-4 py-3">{t('demoDashboard.activity.status')}</th>
          <th className="px-4 py-3">{t('demoDashboard.activity.updatedAt')}</th>
        </tr>
      </thead>
      <tbody>
        {activities.map((activity) => (
          <tr className="border-t border-border-muted" key={activity.id}>
            <td className="px-4 py-3 font-medium text-strong-foreground">
              {activity.title}
            </td>
            <td className="px-4 py-3 text-muted-foreground">
              {activity.owner}
            </td>
            <td className="px-4 py-3">
              <Badge variant={getStatusVariant(activity.status)}>
                {t(`demoDashboard.status.${activity.status}`)}
              </Badge>
            </td>
            <td className="px-4 py-3 text-muted-foreground">
              {activity.updatedAt}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  </div>
)
