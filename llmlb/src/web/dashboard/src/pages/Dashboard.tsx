import { useDashboardViewModel } from '@/viewmodels/useDashboardViewModel'
import { Header } from '@/components/dashboard/Header'
import { OperationsOverview } from '@/components/dashboard/OperationsOverview'
import { EndpointTable } from '@/components/dashboard/EndpointTable'
import { ModelsTable } from '@/components/dashboard/ModelsTable'
import { RequestHistoryTable } from '@/components/dashboard/RequestHistoryTable'
import { LogViewer } from '@/components/dashboard/LogViewer'
import { TokenStatsSection } from '@/components/dashboard/TokenStatsSection'
import { ClientsTab } from '@/components/dashboard/ClientsTab'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogDescription,
} from '@/components/ui/dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import {
  AlertCircle,
  AlertTriangle,
  Globe,
  History,
  FileText,
  BarChart3,
  ArrowUpCircle,
  ExternalLink,
  Loader2,
  RefreshCcw,
  Users,
  Settings,
  Undo2,
  Calendar,
  Clock,
  Zap,
  Package,
  ShieldCheck,
} from 'lucide-react'

export default function Dashboard() {
  const {
    user, isViewer, isAdmin, data, isLoading, error, errorMessage, refetch,
    isLoadingHistory, endpointsData, isLoadingEndpoints, lastRefreshed, fetchTimeMs,
    systemVersion, viewerModels, isLoadingViewerModels, refetchViewerModels,
    modelsView, setModelsView, historyItems, activeTab, setActiveTab, updateBanner: update,
  } = useDashboardViewModel()
  const {
    isApplyingUpdate, isApplyingForceUpdate, isForceUpdateDialogOpen, setIsForceUpdateDialogOpen,
    isCheckingUpdate, isCooldown, isRollbackDialogOpen, setIsRollbackDialogOpen, isRollingBack,
    isSettingsOpen, setIsSettingsOpen, scheduleMode, setScheduleMode, scheduledAt, setScheduledAt,
    isScheduling, drainCountdown, applyTimeoutCountdown,
    updateState, applying, showRestartButton, showForceButton, canApply, canForceApply,
    canCheck, forceUpdateTitle, rollbackAvailable, title, description, link, payloadHint,
    downloadProgress, onCheck, onApply, onForceApply, onRollback, onSchedule, onCancelSchedule,
    scheduleInfo, scheduleDescription, downloadProgressPercent, restartLabel,
  } = update

  const updateBanner = (
    <section className="mb-6">
      <div className="rounded-2xl border border-border/60 bg-card/60 backdrop-blur-xl px-5 py-4 shadow-sm">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-start gap-3">
            <div className="mt-0.5 flex h-9 w-9 items-center justify-center rounded-xl bg-primary/10">
              {applying ? (
                <Loader2 className="h-5 w-5 animate-spin text-primary" />
              ) : (
                <ArrowUpCircle className="h-5 w-5 text-primary" />
              )}
            </div>
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <p className="font-medium leading-6">{title}</p>
                {payloadHint && (
                  <span className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                    {payloadHint}
                  </span>
                )}
              </div>
              {description && (
                <p className="mt-0.5 text-sm text-muted-foreground">
                  {description}
                </p>
              )}
              {downloadProgress && (
                <div className="mt-2 w-64">
                  <Progress value={downloadProgressPercent ?? 0} />
                </div>
              )}
              {updateState === 'draining' && drainCountdown != null && (
                <p className="mt-1 text-xs text-muted-foreground">
                  {`Drain timeout in ${drainCountdown}`}
                </p>
              )}
              {updateState === 'applying' && applyTimeoutCountdown != null && (
                <p className="mt-1 text-xs text-muted-foreground">
                  {`Apply timeout in ${applyTimeoutCountdown}`}
                </p>
              )}
              {scheduleInfo && (
                <p className="mt-1 text-xs text-muted-foreground">
                  {scheduleDescription}
                  <button
                    type="button"
                    className="ml-2 text-destructive hover:underline"
                    onClick={() => void onCancelSchedule()}
                  >
                    Cancel
                  </button>
                </p>
              )}
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            {link && (
              <a
                href={link}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-1 rounded-lg border border-border/60 bg-background/60 px-3 py-2 text-sm hover:bg-background"
              >
                <ExternalLink className="h-4 w-4" />
                Release
              </a>
            )}

            {/* Settings button */}
            {isAdmin && (
              <Dialog open={isSettingsOpen} onOpenChange={setIsSettingsOpen}>
                <DialogTrigger asChild>
                  <Button variant="outline" size="icon" title="Settings">
                    <Settings className="h-4 w-4" />
                  </Button>
                </DialogTrigger>
                <DialogContent className="sm:max-w-lg">
                  <DialogHeader>
                    <DialogTitle>Settings</DialogTitle>
                    <DialogDescription>
                      Configure access control, update scheduling, and history.
                    </DialogDescription>
                  </DialogHeader>
                  <Tabs defaultValue="authentication">
                    <TabsList className="grid w-full grid-cols-3">
                      <TabsTrigger value="authentication">
                        <ShieldCheck className="mr-1.5 h-3.5 w-3.5" />
                        Auth
                      </TabsTrigger>
                      <TabsTrigger value="schedule">
                        <Calendar className="mr-1.5 h-3.5 w-3.5" />
                        Schedule
                      </TabsTrigger>
                      <TabsTrigger value="history">
                        <History className="mr-1.5 h-3.5 w-3.5" />
                        History
                      </TabsTrigger>
                    </TabsList>

                    <TabsContent value="authentication" className="space-y-4 pt-4">
                      <div className="rounded-lg border border-border p-4">
                        <div className="space-y-1">
                          <Label className="text-sm font-medium">Authentication required</Label>
                          <p className="text-sm text-muted-foreground">
                            All external and management APIs always require a valid API key or an authenticated dashboard session. Unauthenticated requests receive 401.
                          </p>
                        </div>
                      </div>
                    </TabsContent>

                    <TabsContent value="schedule" className="space-y-4 pt-4">
                      <div className="space-y-3">
                        <Label
                          className={`flex items-center gap-2 cursor-pointer rounded-lg border p-3 ${scheduleMode === 'immediate' ? 'border-primary bg-primary/5' : 'border-border'}`}
                        >
                          <input
                            type="radio"
                            name="scheduleMode"
                            value="immediate"
                            checked={scheduleMode === 'immediate'}
                            onChange={() => setScheduleMode('immediate')}
                            className="accent-primary"
                          />
                          <Zap className="h-4 w-4" />
                          <span>Immediate</span>
                        </Label>
                        <Label
                          className={`flex items-center gap-2 cursor-pointer rounded-lg border p-3 ${scheduleMode === 'idle' ? 'border-primary bg-primary/5' : 'border-border'}`}
                        >
                          <input
                            type="radio"
                            name="scheduleMode"
                            value="idle"
                            checked={scheduleMode === 'idle'}
                            onChange={() => setScheduleMode('idle')}
                            className="accent-primary"
                          />
                          <Clock className="h-4 w-4" />
                          <span>When idle (in_flight = 0)</span>
                        </Label>
                        <Label
                          className={`flex items-center gap-2 cursor-pointer rounded-lg border p-3 ${scheduleMode === 'scheduled' ? 'border-primary bg-primary/5' : 'border-border'}`}
                        >
                          <input
                            type="radio"
                            name="scheduleMode"
                            value="scheduled"
                            checked={scheduleMode === 'scheduled'}
                            onChange={() => setScheduleMode('scheduled')}
                            className="accent-primary"
                          />
                          <Calendar className="h-4 w-4" />
                          <span>Scheduled time</span>
                        </Label>
                        {scheduleMode === 'scheduled' && (
                          <Input
                            type="datetime-local"
                            value={scheduledAt}
                            onChange={(e) => setScheduledAt(e.target.value)}
                            className="mt-2"
                          />
                        )}
                      </div>
                      <Button
                        className="w-full"
                        onClick={() => void onSchedule()}
                        disabled={isScheduling || (scheduleMode === 'scheduled' && !scheduledAt)}
                      >
                        {isScheduling && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                        {scheduleMode === 'immediate' ? 'Apply now' : 'Schedule'}
                      </Button>
                    </TabsContent>

                    <TabsContent value="history" className="pt-4">
                      <p className="text-sm text-muted-foreground">
                        Update history will be available after the first update completes.
                      </p>
                    </TabsContent>
                  </Tabs>
                </DialogContent>
              </Dialog>
            )}

            <Button
              variant="outline"
              onClick={onCheck}
              disabled={!canCheck || isCheckingUpdate || isApplyingUpdate || isApplyingForceUpdate}
              title={
                !isAdmin
                  ? 'Admin role is required'
                  : applying
                    ? 'Update is in progress'
                    : isCooldown
                      ? 'Please wait before checking again'
                      : undefined
              }
            >
              {isCheckingUpdate ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <RefreshCcw className="h-4 w-4" />
              )}
              Check for updates
            </Button>
            {showRestartButton && (
              <Button
                onClick={onApply}
                disabled={!canApply || isApplyingUpdate || isApplyingForceUpdate || applying}
                title={
                  !isAdmin
                    ? 'Admin role is required'
                    : applying
                      ? 'Update is in progress'
                      : undefined
                }
              >
                {isApplyingUpdate || applying ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <ArrowUpCircle className="h-4 w-4" />
                )}
                {restartLabel}
              </Button>
            )}
            {showForceButton && (
              <AlertDialog
                open={isForceUpdateDialogOpen}
                onOpenChange={setIsForceUpdateDialogOpen}
              >
                <AlertDialogTrigger asChild>
                  <Button
                    variant="destructive"
                    disabled={!canForceApply || isApplyingUpdate || isApplyingForceUpdate}
                    title={forceUpdateTitle}
                  >
                    {isApplyingForceUpdate ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <AlertTriangle className="h-4 w-4" />
                    )}
                    Force update now
                  </Button>
                </AlertDialogTrigger>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>Force update now?</AlertDialogTitle>
                    <AlertDialogDescription>
                      In-flight inference requests will be terminated immediately and llmlb will restart. Use this only for urgent maintenance.
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogCancel disabled={isApplyingForceUpdate}>Cancel</AlertDialogCancel>
                    <AlertDialogAction
                      disabled={isApplyingForceUpdate}
                      onClick={(event) => {
                        event.preventDefault()
                        void onForceApply()
                      }}
                    >
                      {isApplyingForceUpdate ? (
                        <>
                          <Loader2 className="h-4 w-4 animate-spin" />
                          Applying...
                        </>
                      ) : (
                        'Force update'
                      )}
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            )}

            {/* Rollback button */}
            {isAdmin && rollbackAvailable && (
              <AlertDialog
                open={isRollbackDialogOpen}
                onOpenChange={setIsRollbackDialogOpen}
              >
                <AlertDialogTrigger asChild>
                  <Button
                    variant="outline"
                    disabled={!rollbackAvailable || applying}
                    title={
                      !rollbackAvailable
                        ? 'No previous version available'
                        : applying
                          ? 'Update is in progress'
                          : undefined
                    }
                  >
                    <Undo2 className="h-4 w-4" />
                    Rollback to previous version
                  </Button>
                </AlertDialogTrigger>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>Rollback to previous version?</AlertDialogTitle>
                    <AlertDialogDescription>
                      This will restore the previous binary and restart llmlb. In-flight requests will complete before the rollback is applied.
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogCancel disabled={isRollingBack}>Cancel</AlertDialogCancel>
                    <AlertDialogAction
                      disabled={isRollingBack}
                      onClick={(event) => {
                        event.preventDefault()
                        void onRollback()
                      }}
                    >
                      {isRollingBack ? (
                        <>
                          <Loader2 className="h-4 w-4 animate-spin" />
                          Rolling back...
                        </>
                      ) : (
                        'Rollback'
                      )}
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            )}
          </div>
        </div>
      </div>
    </section>
  )


  if (error) {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-4 text-center">
          <div className="flex h-16 w-16 items-center justify-center rounded-full bg-destructive/10">
            <AlertCircle className="h-8 w-8 text-destructive" />
          </div>
          <div>
            <h2 className="text-lg font-semibold">Failed to load dashboard</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {errorMessage}
            </p>
          </div>
          <Button variant="link" onClick={() => refetch()}>
            Try again
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-background">
      {/* Background Grid */}
      <div className="fixed inset-0 bg-grid opacity-20 pointer-events-none" />

      {/* Header */}
      <Header
        user={user}
        isConnected={!error}
        lastRefreshed={lastRefreshed}
        fetchTimeMs={fetchTimeMs}
        systemVersion={systemVersion}
        updateState={update.updateState}
        updateLatest={update.updateLatest}
        minimalViewer={isViewer}
      />

      {/* Main Content */}
      <main className="relative mx-auto max-w-[1600px] px-4 py-6 sm:px-6 lg:px-8">
        {!isViewer && updateBanner}
        <section className="mb-8">
          <OperationsOverview overview={data} isLoading={isLoading} />
        </section>

        {isViewer ? (
          <section className="mb-8">
            <ModelsTable
              models={viewerModels || []}
              endpoints={endpointsData || []}
              isLoading={isLoadingViewerModels}
              onRefresh={() => {
                void refetchViewerModels()
              }}
              view={modelsView}
              onViewChange={setModelsView}
              viewerMode
            />
          </section>
        ) : (
          <Tabs value={activeTab} onValueChange={setActiveTab} className="space-y-6">
            <TabsList className="grid w-full grid-cols-6 lg:w-auto lg:inline-grid">
              <TabsTrigger value="endpoints" className="gap-2">
                <Globe className="h-4 w-4" />
                <span className="hidden sm:inline">Endpoints</span>
              </TabsTrigger>
              <TabsTrigger value="models" className="gap-2">
                <Package className="h-4 w-4" />
                <span className="hidden sm:inline">Models</span>
              </TabsTrigger>
              <TabsTrigger value="statistics" className="gap-2">
                <BarChart3 className="h-4 w-4" />
                <span className="hidden sm:inline">Usage</span>
              </TabsTrigger>
              <TabsTrigger value="history" className="gap-2">
                <History className="h-4 w-4" />
                <span className="hidden sm:inline">Requests</span>
              </TabsTrigger>
              <TabsTrigger value="clients" className="gap-2">
                <Users className="h-4 w-4" />
                <span className="hidden sm:inline">Traffic</span>
              </TabsTrigger>
              <TabsTrigger value="logs" className="gap-2">
                <FileText className="h-4 w-4" />
                <span className="hidden sm:inline">System</span>
              </TabsTrigger>
            </TabsList>

            <TabsContent value="endpoints" className="animate-fade-in">
              <EndpointTable
                endpoints={endpointsData || []}
                isLoading={isLoadingEndpoints}
              />
            </TabsContent>

            <TabsContent value="models" className="animate-fade-in">
              <ModelsTable
                models={viewerModels || []}
                endpoints={endpointsData || []}
                isLoading={isLoadingViewerModels}
                onRefresh={() => { void refetchViewerModels() }}
                view={modelsView}
                onViewChange={setModelsView}
              />
            </TabsContent>

            <TabsContent value="statistics" className="animate-fade-in">
              <TokenStatsSection />
            </TabsContent>

            <TabsContent value="history" className="animate-fade-in">
              <RequestHistoryTable
                history={historyItems}
                isLoading={isLoadingHistory}
              />
            </TabsContent>

            <TabsContent value="clients" className="animate-fade-in">
              <ClientsTab />
            </TabsContent>

            <TabsContent value="logs" className="animate-fade-in">
              <LogViewer />
            </TabsContent>
          </Tabs>
        )}
      </main>
    </div>
  )
}
