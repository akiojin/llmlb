import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { modelsApi, type RegisteredModelView, type ModelsView } from '@/lib/api/models'
import { systemApi, type VersionResponse } from '@/lib/api/system'
import type { RequestHistoryItem } from '@/lib/api/dashboard'
import { queryKeys } from '@/lib/queryKeys'
import { useAuth } from '@/hooks/useAuth'
import { useDashboardWebSocket } from '@/hooks/useWebSocket'
import { useDashboardDataViewModel, type DashboardDataViewModel } from './useDashboardDataViewModel'
import { useSystemUpdateViewModel, type SystemUpdateViewModel } from './useSystemUpdateViewModel'

export interface DashboardViewModel extends Omit<DashboardDataViewModel, 'systemInfo' | 'requestResponsesData'> {
  user: ReturnType<typeof useAuth>['user']
  isViewer: boolean
  isAdmin: boolean
  systemVersion: string | null
  viewerModels: RegisteredModelView[]
  isLoadingViewerModels: boolean
  refetchViewerModels: () => void
  modelsView: ModelsView
  setModelsView: (view: ModelsView) => void
  historyItems: RequestHistoryItem[]
  activeTab: string
  setActiveTab: (tab: string) => void
  updateBanner: SystemUpdateViewModel
  errorMessage: string
}

const DASHBOARD_TABS = ['endpoints', 'models', 'statistics', 'history', 'clients', 'logs']

function readInitialTab(): string {
  const tabParam = new URLSearchParams(window.location.search).get('tab')
  return tabParam && DASHBOARD_TABS.includes(tabParam) ? tabParam : 'endpoints'
}

/** Own the page's connection, queries, derived state and commands; the page only renders. */
export function useDashboardViewModel(): DashboardViewModel {
  const { user } = useAuth()
  const isViewer = user?.role === 'viewer'
  const isAdmin = user?.role === 'admin'
  const { isConnected: wsConnected } = useDashboardWebSocket({ enabled: !isViewer })
  const [activeTab, setActiveTab] = useState(readInitialTab)
  // When WebSocket is connected, reduce polling frequency
  const pollingInterval = wsConnected ? 10000 : 5000

  const { systemInfo, requestResponsesData, ...dashboard } = useDashboardDataViewModel({ pollingInterval, isViewer })

  // Bug 3: /api/version は認証不要・軽量なので全ロールで常時取得し、
  // systemInfo 未取得時のフォールバックにする
  const { data: versionData } = useQuery<VersionResponse>({
    queryKey: queryKeys.version(),
    queryFn: () => systemApi.getVersion(),
    refetchInterval: pollingInterval,
  })

  // admin: systemInfo.version 優先, フォールバック: versionData.version
  const systemVersion = systemInfo?.version ?? versionData?.version ?? null

  // US-029: モデル一覧の表示モード（canonical 集約 / detail 全 variant）
  const [modelsView, setModelsView] = useState<ModelsView>('canonical')

  const {
    data: viewerModels,
    isLoading: isLoadingViewerModels,
    refetch: refetchViewerModels,
  } = useQuery<RegisteredModelView[]>({
    queryKey: queryKeys.viewerModels(modelsView),
    queryFn: () => modelsApi.getRegistered(modelsView),
    refetchInterval: pollingInterval,
  })

  // Map RequestResponseRecord to RequestHistoryItem
  const historyItems: RequestHistoryItem[] = useMemo(() => {
    if (!requestResponsesData?.records) return []
    return requestResponsesData.records.map((record) => ({
      request_id: record.id,
      timestamp: record.timestamp,
      model: record.model,
      node_id: record.endpoint_id,
      node_name: record.endpoint_name,
      status: record.status.type,
      duration_ms: record.duration_ms,
      error: record.status.type === 'error' ? record.status.message : undefined,
      request_body: record.request_body,
      response_body: record.response_body,
      client_ip: record.client_ip,
    }))
  }, [requestResponsesData])

  const updateBanner = useSystemUpdateViewModel(systemInfo, isAdmin)
  return {
    ...dashboard, user, isViewer, isAdmin, systemVersion,
    viewerModels: viewerModels ?? [], isLoadingViewerModels,
    refetchViewerModels: () => { void refetchViewerModels() },
    modelsView, setModelsView, historyItems, activeTab, setActiveTab, updateBanner,
    errorMessage: dashboard.error instanceof Error ? dashboard.error.message : 'An error occurred',
  }
}
