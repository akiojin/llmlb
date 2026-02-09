import { type Page, type Locator, expect } from '@playwright/test';
import { DashboardSelectors } from '../helpers/selectors';

/**
 * Page Object Model for LLM Router Dashboard
 */
export class DashboardPage {
  readonly page: Page;

  // Header Controls
  readonly themeToggle: Locator;
  readonly playgroundButton: Locator;
  readonly apiKeysButton: Locator;
  readonly refreshButton: Locator;
  readonly connectionStatus: Locator;

  // Stats
  readonly totalEndpoints: Locator;
  readonly totalRequests: Locator;
  readonly successRate: Locator;
  readonly averageResponseTime: Locator;

  // Models - Tabs
  // NOTE: Model Hub タブは SPEC-6cd7f960 により廃止されました
  readonly localTab: Locator;
  readonly localModelsList: Locator;
  // Models - Local tab elements
  readonly localSearch: Locator;
  readonly registerButton: Locator;
  // Models - Registration dialog
  readonly registerModal: Locator;
  readonly registerRepo: Locator;
  readonly registerSubmit: Locator;

  // Nodes
  readonly nodesBody: Locator;
  readonly filterStatus: Locator;
  readonly filterQuery: Locator;
  readonly exportJson: Locator;
  readonly exportCsv: Locator;

  // Modals
  readonly chatModal: Locator;
  readonly chatClose: Locator;
  readonly apiKeysModal: Locator;

  constructor(page: Page) {
    this.page = page;

    // Header
    this.themeToggle = page.locator(DashboardSelectors.header.themeToggle);
    this.playgroundButton = page.locator(DashboardSelectors.header.playgroundButton);
    this.apiKeysButton = page.locator(DashboardSelectors.header.apiKeysButton);
    this.refreshButton = page.locator(DashboardSelectors.header.refreshButton);
    this.connectionStatus = page.locator(DashboardSelectors.header.connectionStatus);

    // Stats
    this.totalEndpoints = page.locator(DashboardSelectors.stats.totalEndpoints);
    this.totalRequests = page.locator(DashboardSelectors.stats.totalRequests);
    this.successRate = page.locator(DashboardSelectors.stats.successRate);
    this.averageResponseTime = page.locator(DashboardSelectors.stats.averageResponseTime);

    // Models - Tabs
    // NOTE: Model Hub タブは SPEC-6cd7f960 により廃止されました
    this.localTab = page.locator(DashboardSelectors.models.localTab);
    this.localModelsList = page.locator(DashboardSelectors.models.localModelsList);
    // Models - Local tab elements
    this.localSearch = page.locator(DashboardSelectors.models.localSearch);
    this.registerButton = page.locator(DashboardSelectors.models.registerButton);
    // Models - Registration dialog
    this.registerModal = page.locator(DashboardSelectors.models.registerModal);
    this.registerRepo = page.locator(DashboardSelectors.models.registerRepo);
    this.registerSubmit = page.locator(DashboardSelectors.models.registerSubmit);

    // Nodes
    this.nodesBody = page.locator(DashboardSelectors.nodes.nodesBody);
    this.filterStatus = page.locator(DashboardSelectors.nodes.filterStatus);
    this.filterQuery = page.locator(DashboardSelectors.nodes.filterQuery);
    this.exportJson = page.locator(DashboardSelectors.nodes.exportJson);
    this.exportCsv = page.locator(DashboardSelectors.nodes.exportCsv);

    // Modals
    this.chatModal = page.locator(DashboardSelectors.modals.chatModal);
    this.chatClose = page.locator(DashboardSelectors.modals.chatClose);
    this.apiKeysModal = page.locator(DashboardSelectors.modals.apiKeysModal);
  }

  async goto() {
    await this.page.goto('/dashboard');
    // Wait for page to settle (use 'load' instead of 'networkidle' due to WebSocket connections)
    await this.page.waitForLoadState('load');
    // Wait a moment for any JavaScript redirects
    await this.page.waitForTimeout(500);
    // Handle login if redirected to login page or login form appears after redirect
    const loginForm = this.page
      .locator('form')
      .filter({ hasText: 'Sign in' });
    for (let attempt = 0; attempt < 2; attempt += 1) {
      const isLoginUrl = this.page.url().includes('login');
      const hasLoginForm = await loginForm
        .isVisible({ timeout: 2000 })
        .catch(() => false);
      if (isLoginUrl || hasLoginForm) {
        await this.login();
      }
      try {
        // Wait for dashboard content to be visible
        await this.page.waitForSelector('#theme-toggle', { timeout: 10000 });
        return;
      } catch (error) {
        if (attempt === 0) {
          continue;
        }
        throw error;
      }
    }
  }

  async gotoModels() {
    await this.goto();
    // Navigate to Models tab
    await this.page.click('button[role="tab"]:has-text("Models")');
    await this.page.waitForTimeout(500);
  }

  async gotoLocalModels() {
    await this.gotoModels();
    // Click Local tab
    await this.localTab.click();
    await this.page.waitForTimeout(300);
  }

  /**
   * @deprecated Model Hub タブは SPEC-6cd7f960 により廃止されました
   * Local タブで登録ダイアログを使用してください
   */
  async gotoModelHub() {
    // Model Hub タブは廃止されたため、Local タブに遷移
    await this.gotoLocalModels();
  }

  async login(username = 'admin', password = 'test') {
    await this.page.fill('#username', username);
    await this.page.fill('#password', password);
    await this.page.click('button[type="submit"]');
    // Wait for redirect to dashboard (the URL will NOT contain 'login' after successful login)
    await this.page.waitForFunction(() => !window.location.href.includes('login'), { timeout: 10000 });
    // Use 'load' instead of 'networkidle' due to WebSocket connections
    await this.page.waitForLoadState('load');
  }

  async toggleTheme() {
    await this.themeToggle.click();
  }

  /**
   * Opens the Playground in a new tab.
   * Note: The Playground is a separate page, not a modal.
   * @returns The new page object for the Playground tab
   */
  async openPlayground() {
    const [newPage] = await Promise.all([
      this.page.context().waitForEvent('page'),
      this.playgroundButton.click(),
    ]);
    await newPage.waitForLoadState('networkidle');
    return newPage;
  }

  /**
   * @deprecated Playground is now a separate page, not a modal
   */
  async closePlayground() {
    // No-op - Playground is a separate page now
  }

  async openApiKeys() {
    await this.apiKeysButton.click();
    await expect(this.apiKeysModal).toBeVisible();
  }

  async refresh() {
    await this.refreshButton.click();
  }

  async filterNodesByStatus(status: 'all' | 'online' | 'offline') {
    await this.filterStatus.selectOption(status);
  }

  async searchNodes(query: string) {
    await this.filterQuery.fill(query);
  }

  /**
   * Search local models
   */
  async searchLocalModels(query: string) {
    await this.localSearch.fill(query);
    await this.page.waitForTimeout(300);
  }

  /**
   * @deprecated Use searchLocalModels instead
   */
  async searchHubModels(query: string) {
    await this.searchLocalModels(query);
  }

  /**
   * Open the registration dialog and register a model by repo
   */
  async registerModelByRepo(repo: string, filename?: string) {
    // Open register dialog
    await this.registerButton.click();
    await this.page.waitForTimeout(300);

    // Fill in the repo
    await this.registerRepo.fill(repo);

    // Fill in filename if provided
    if (filename) {
      const filenameInput = this.page.locator(DashboardSelectors.models.registerFilename);
      await filenameInput.fill(filename);
    }

    // Submit
    await this.registerSubmit.click();
    await this.page.waitForTimeout(500);
  }

  /**
   * @deprecated Model Hub は廃止されました。registerModelByRepo を使用してください
   */
  async registerModel(modelId: string) {
    // Model Hub は廃止されたため、repo として登録
    await this.registerModelByRepo(modelId);
  }

  /**
   * Get count of local models displayed
   */
  async getLocalModelCount(): Promise<number> {
    const cards = this.localModelsList.locator('.overflow-hidden');
    return cards.count();
  }

  /**
   * @deprecated Model Hub は廃止されました。getLocalModelCount を使用してください
   */
  async getHubModelCount(): Promise<number> {
    return this.getLocalModelCount();
  }

  async getConnectionStatus(): Promise<string> {
    return (await this.connectionStatus.textContent()) ?? '';
  }

  async getTotalEndpoints(): Promise<string> {
    return (await this.totalEndpoints.textContent()) ?? '-';
  }

  async getNodeCount(): Promise<number> {
    const rows = this.nodesBody.locator('tr:not(.empty-row)');
    return rows.count();
  }

  async sortBy(column: 'machine' | 'status' | 'uptime' | 'total') {
    const selector = `th[data-sort="${column}"]`;
    await this.page.click(selector);
  }
}
