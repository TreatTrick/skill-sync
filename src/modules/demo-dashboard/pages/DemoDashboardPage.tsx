import { useQuery } from '@tanstack/react-query'
import { RefreshCw } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'

import { appEventBus } from '@/shared/lib'
import { t } from '@/shared/i18n'

import { getDashboardData } from '../api/getDashboardData'
import { ActivityTable } from '../components/ActivityTable'
import { MetricCard } from '../components/MetricCard'
import { filterActivitiesByStatus } from '../lib/filterActivities'
import {
  type DashboardStatusFilter,
  useDashboardFilterStore,
} from '../stores/dashboardFilterStore'

const statusFilters: DashboardStatusFilter[] = ['all', 'running', 'done']

export const DemoDashboardPage = () => {
  const [notification, setNotification] = useState('')
  const { statusFilter, setStatusFilter } = useDashboardFilterStore()
  const dashboardQuery = useQuery({
    queryKey: ['dashboard-data'],
    queryFn: getDashboardData,
  })

  useEffect(() => {
    const off = appEventBus.on('notification', (event) => {
      setNotification(event.message)
    })

    return off
  }, [])

  const filteredActivities = useMemo(
    () =>
      filterActivitiesByStatus(
        dashboardQuery.data?.activities ?? [],
        statusFilter,
      ),
    [dashboardQuery.data?.activities, statusFilter],
  )

  const handleRefresh = () => {
    void dashboardQuery.refetch()
    appEventBus.emit('notification', {
      message: t('demoDashboard.toast.refreshed'),
    })
  }

  return (
    <section className="grid gap-5">
      <div className="flex flex-col justify-between gap-3 rounded-lg border border-border bg-surface p-4 sm:flex-row sm:items-start">
        <div>
          <h2 className="text-lg font-bold text-strong-foreground">
            {t('demoDashboard.title')}
          </h2>
          <p className="mt-1 max-w-3xl text-sm leading-6 text-muted-foreground">
            {t('demoDashboard.description')}
          </p>
          {notification ? (
            <p className="mt-3 text-sm font-medium text-success">
              {notification}
            </p>
          ) : null}
        </div>
        <button
          className="inline-flex h-9 w-full items-center justify-center gap-2 rounded-lg bg-primary px-3 text-sm font-bold text-primary-foreground sm:w-auto"
          onClick={handleRefresh}
          type="button"
        >
          <RefreshCw className="size-4" />
          {t('common.actions.refresh')}
        </button>
      </div>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {(dashboardQuery.data?.metrics ?? []).map((metric) => (
          <MetricCard key={metric.id} metric={metric} />
        ))}
      </div>

      <div className="grid gap-3 rounded-lg border border-border bg-surface p-4">
        <div className="flex flex-col justify-between gap-3 sm:flex-row sm:flex-wrap sm:items-center">
          <h2 className="text-base font-bold text-strong-foreground">
            {t('demoDashboard.activity.title')}
          </h2>
          <div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap">
            {statusFilters.map((filter) => (
              <button
                className={[
                  'h-8 rounded-lg border px-3 text-sm font-medium',
                  statusFilter === filter
                    ? 'border-primary bg-primary-muted text-primary-muted-foreground'
                    : 'border-border bg-surface text-foreground hover:bg-surface-hover',
                ].join(' ')}
                key={filter}
                onClick={() => setStatusFilter(filter)}
                type="button"
              >
                {t(`demoDashboard.filters.${filter}`)}
              </button>
            ))}
          </div>
        </div>

        <ActivityTable activities={filteredActivities} />
      </div>
    </section>
  )
}
