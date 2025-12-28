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
  readonly totalNodes: Locator;
  readonly totalRequests: Locator;
  readonly successRate: Locator;
  readonly averageResponseTime: Locator;

  // Models - Tabs
  readonly localTab: Locator;
  readonly hubTab: Locator;
  readonly localModelsList: Locator;
  readonly hubModelsList: Locator;
  // Models - Hub
  readonly hubSearch: Locator;
  readonly hubModelCards: Locator;

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
    this.totalNodes = page.locator(DashboardSelectors.stats.totalNodes);
    this.totalRequests = page.locator(DashboardSelectors.stats.totalRequests);
    this.successRate = page.locator(DashboardSelectors.stats.successRate);
    this.averageResponseTime = page.locator(DashboardSelectors.stats.averageResponseTime);

    // Models - Tabs
    this.localTab = page.locator(DashboardSelectors.models.localTab);
    this.hubTab = page.locator(DashboardSelectors.models.hubTab);
    this.localModelsList = page.locator(DashboardSelectors.models.localModelsList);
    this.hubModelsList = page.locator(DashboardSelectors.models.hubModelsList);
    // Models - Hub
    this.hubSearch = page.locator(DashboardSelectors.models.hubSearch);
    this.hubModelCards = page.locator(DashboardSelectors.models.hubModelCard);

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
    // Wait for page to settle
    await this.page.waitForLoadState('networkidle');
    // Handle login if redirected to login page
    if (this.page.url().includes('login')) {
      await this.login();
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

  async gotoModelHub() {
    await this.gotoModels();
    // Click Model Hub tab
    await this.hubTab.click();
    await this.page.waitForTimeout(300);
  }

  async login(username = 'admin', password = 'test') {
    await this.page.fill('#username', username);
    await this.page.fill('#password', password);
    await this.page.click('button[type="submit"]');
    // Wait for redirect to dashboard (the URL will NOT contain 'login' after successful login)
    await this.page.waitForFunction(() => !window.location.href.includes('login'), { timeout: 10000 });
    await this.page.waitForLoadState('networkidle');
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

  async searchHubModels(query: string) {
    await this.hubSearch.fill(query);
    await this.page.waitForTimeout(300);
  }

  async pullModel(modelId: string) {
    // Find the model card and click Pull button
    const modelCard = this.page.locator(`[data-model-id="${modelId}"]`);
    const pullButton = modelCard.locator('button:has-text("Pull")');
    await pullButton.click();
    await this.page.waitForTimeout(500);
  }

  async getHubModelCount(): Promise<number> {
    return this.hubModelCards.count();
  }

  async getConnectionStatus(): Promise<string> {
    return (await this.connectionStatus.textContent()) ?? '';
  }

  async getTotalNodes(): Promise<string> {
    return (await this.totalNodes.textContent()) ?? '-';
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
