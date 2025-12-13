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
  readonly onlineNodes: Locator;
  readonly offlineNodes: Locator;
  readonly totalRequests: Locator;

  // Models
  readonly hfRegisterUrl: Locator;
  readonly hfRegisterSubmit: Locator;
  readonly registeredModelsList: Locator;

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
    this.onlineNodes = page.locator(DashboardSelectors.stats.onlineNodes);
    this.offlineNodes = page.locator(DashboardSelectors.stats.offlineNodes);
    this.totalRequests = page.locator(DashboardSelectors.stats.totalRequests);

    // Models
    this.hfRegisterUrl = page.locator(DashboardSelectors.models.hfRegisterUrl);
    this.hfRegisterSubmit = page.locator(DashboardSelectors.models.hfRegisterSubmit);
    this.registeredModelsList = page.locator(DashboardSelectors.models.registeredModelsList);

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

  async openPlayground() {
    await this.playgroundButton.click();
    await expect(this.chatModal).toBeVisible();
  }

  async closePlayground() {
    await this.chatClose.click();
    await expect(this.chatModal).toBeHidden();
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

  async registerModelUrl(url: string) {
    await this.hfRegisterUrl.fill(url);
    await this.hfRegisterSubmit.click();
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
