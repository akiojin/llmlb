/**
 * CSS Selectors for LLM Router Dashboard and Playground
 * Centralized selector definitions for maintainability
 */

export const DashboardSelectors = {
  // Header Controls
  header: {
    themeToggle: '#theme-toggle',
    playgroundButton: '#chat-open',
    apiKeysButton: '#api-keys-button',
    refreshButton: '#refresh-button',
    connectionStatus: '#connection-status',
    lastRefreshed: '#last-refreshed',
    refreshMetrics: '#refresh-metrics',
  },

  // Stats Grid
  stats: {
    totalNodes: '[data-stat="total-nodes"]',
    onlineNodes: '[data-stat="online-nodes"]',
    offlineNodes: '[data-stat="offline-nodes"]',
    totalRequests: '[data-stat="total-requests"]',
    successfulRequests: '[data-stat="successful-requests"]',
    failedRequests: '[data-stat="failed-requests"]',
    averageResponseTime: '[data-stat="average-response-time-ms"]',
    averageGpuUsage: '[data-stat="average-gpu-usage"]',
    averageGpuMemory: '[data-stat="average-gpu-memory-usage"]',
    activeRequests: '[data-stat="total-active-requests"]',
  },

  // Models Tab
  models: {
    hfRegisterUrl: '#hf-register-url',
    hfRegisterSubmit: '#hf-register-url-submit',
    // Unified local models view (registered + downloading)
    localModelsList: '#local-models-list',
    // Legacy selectors (hidden but kept for backward compatibility)
    registeredModelsList: '#local-models-list',
    registeringTasksList: '#local-models-list',
    downloadTasksList: '#local-models-list',
    convertTasksList: '#local-models-list',
    // Local model card selectors
    localModelCard: '.local-model-card',
    localModelCardDownloading: '.local-model-card--downloading',
    localModelCardRegistered: '.local-model-card--registered',
    convertModal: '#convert-modal',
    convertModalClose: '#convert-modal-close',
    convertRepo: '#convert-repo',
    convertFilename: '#convert-filename',
    convertSubmit: '#convert-submit',
  },

  // Nodes Tab
  nodes: {
    nodesBody: '#nodes-body',
    filterStatus: '#filter-status',
    filterQuery: '#filter-query',
    selectAll: '#select-all',
    exportJson: '#export-json',
    exportCsv: '#export-csv',
    pagePrev: '#page-prev',
    pageNext: '#page-next',
    pageInfo: '#page-info',
    sortMachine: 'th[data-sort="machine"]',
    sortStatus: 'th[data-sort="status"]',
    sortUptime: 'th[data-sort="uptime"]',
    sortTotal: 'th[data-sort="total"]',
  },

  // Request History Tab
  history: {
    historyTbody: '#request-history-tbody',
    filterModel: '#filter-history-model',
    perPage: '#history-per-page',
    pagePrev: '#history-page-prev',
    pageNext: '#history-page-next',
    pageInfo: '#history-page-info',
    exportCsv: '#export-history-csv',
  },

  // Logs Tab
  logs: {
    coordinatorList: '#logs-coordinator-list',
    coordinatorRefresh: '#logs-coordinator-refresh',
    nodeSelect: '#logs-node-select',
    nodeList: '#logs-node-list',
    nodeRefresh: '#logs-node-refresh',
  },

  // Modals
  modals: {
    nodeModal: '#node-modal',
    nodeModalClose: '#node-modal-close',
    nodeModalSave: '#node-modal-save',
    nodeModalDisconnect: '#node-modal-disconnect',
    nodeModalDelete: '#node-modal-delete',
    requestModal: '#request-modal',
    requestModalClose: '#request-modal-close',
    apiKeysModal: '#api-keys-modal',
    apiKeysModalClose: '.modal__close',
    apiKeyName: '#api-key-name',
    createApiKey: '#create-api-key',
    copyApiKey: '#copy-api-key',
    chatModal: '#chat-modal',
    chatClose: '#chat-close',
  },

  // Error Banner
  errorBanner: '#error-banner',
  errorBannerClose: '#error-banner-close',
};

export const PlaygroundSelectors = {
  // Sidebar
  sidebar: {
    container: '#sidebar',
    toggle: '#sidebar-toggle',
    toggleMobile: '#sidebar-toggle-mobile',
    newChat: '#new-chat',
    sessionList: '#session-list',
  },

  // Header
  header: {
    modelSelect: '#model-select',
    routerStatus: '#router-status',
    settingsToggle: '#settings-toggle',
  },

  // Chat
  chat: {
    container: '.chat-container',
    messages: '#chat-log',
    input: '#chat-input',
    sendButton: '#send-button',
    stopButton: '#stop-button',
    form: '#chat-form',
    welcome: '.chat-welcome',
  },

  // Messages
  messages: {
    user: '.message--user',
    assistant: '.message--assistant',
    text: '.message-text',
  },

  // Error Banner
  errorBanner: '#error-banner',
  errorMessage: '#error-message',
  errorClose: '#error-close',

  // Settings Modal
  settings: {
    modal: '#settings-modal',
    close: '#modal-close',
    providerLocal: '.provider-btn[data-provider="local"]',
    providerCloud: '.provider-btn[data-provider="cloud"]',
    providerAll: '.provider-btn[data-provider="all"]',
    apiKeyInput: '#api-key-input',
    streamToggle: '#stream-toggle',
    appendSystem: '#append-system',
    systemPrompt: '#system-prompt',
    resetChat: '#reset-chat',
    copyCurl: '#copy-curl',
  },
};
