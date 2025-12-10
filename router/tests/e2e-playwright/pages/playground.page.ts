import { type Page, type Locator, expect } from '@playwright/test';
import { PlaygroundSelectors } from '../helpers/selectors';

/**
 * Page Object Model for LLM Router Playground
 */
export class PlaygroundPage {
  readonly page: Page;

  // Sidebar
  readonly sidebar: Locator;
  readonly sidebarToggle: Locator;
  readonly newChatButton: Locator;
  readonly sessionList: Locator;

  // Header
  readonly modelSelect: Locator;
  readonly routerStatus: Locator;
  readonly settingsToggle: Locator;

  // Chat
  readonly chatContainer: Locator;
  readonly chatInput: Locator;
  readonly sendButton: Locator;
  readonly stopButton: Locator;

  // Settings Modal
  readonly settingsModal: Locator;
  readonly settingsClose: Locator;
  readonly providerLocal: Locator;
  readonly providerCloud: Locator;
  readonly providerAll: Locator;
  readonly apiKeyInput: Locator;
  readonly streamToggle: Locator;
  readonly systemPrompt: Locator;
  readonly resetChat: Locator;
  readonly copyCurl: Locator;

  // Error
  readonly errorBanner: Locator;
  readonly errorClose: Locator;

  constructor(page: Page) {
    this.page = page;

    // Sidebar
    this.sidebar = page.locator(PlaygroundSelectors.sidebar.container);
    this.sidebarToggle = page.locator(PlaygroundSelectors.sidebar.toggle);
    this.newChatButton = page.locator(PlaygroundSelectors.sidebar.newChat);
    this.sessionList = page.locator(PlaygroundSelectors.sidebar.sessionList);

    // Header
    this.modelSelect = page.locator(PlaygroundSelectors.header.modelSelect);
    this.routerStatus = page.locator(PlaygroundSelectors.header.routerStatus);
    this.settingsToggle = page.locator(PlaygroundSelectors.header.settingsToggle);

    // Chat
    this.chatContainer = page.locator(PlaygroundSelectors.chat.container);
    this.chatInput = page.locator(PlaygroundSelectors.chat.input);
    this.sendButton = page.locator(PlaygroundSelectors.chat.sendButton);
    this.stopButton = page.locator(PlaygroundSelectors.chat.stopButton);

    // Settings Modal
    this.settingsModal = page.locator(PlaygroundSelectors.settings.modal);
    this.settingsClose = page.locator(PlaygroundSelectors.settings.close);
    this.providerLocal = page.locator(PlaygroundSelectors.settings.providerLocal);
    this.providerCloud = page.locator(PlaygroundSelectors.settings.providerCloud);
    this.providerAll = page.locator(PlaygroundSelectors.settings.providerAll);
    this.apiKeyInput = page.locator(PlaygroundSelectors.settings.apiKeyInput);
    this.streamToggle = page.locator(PlaygroundSelectors.settings.streamToggle);
    this.systemPrompt = page.locator(PlaygroundSelectors.settings.systemPrompt);
    this.resetChat = page.locator(PlaygroundSelectors.settings.resetChat);
    this.copyCurl = page.locator(PlaygroundSelectors.settings.copyCurl);

    // Error
    this.errorBanner = page.locator(PlaygroundSelectors.errorBanner);
    this.errorClose = page.locator(PlaygroundSelectors.errorClose);
  }

  async goto() {
    await this.page.goto('/playground');
  }

  async toggleSidebar() {
    await this.sidebarToggle.click();
  }

  async newChat() {
    await this.newChatButton.click();
  }

  async selectModel(modelName: string) {
    await this.modelSelect.selectOption({ label: modelName });
  }

  async openSettings() {
    await this.settingsToggle.click();
    await expect(this.settingsModal).toBeVisible();
  }

  async closeSettings() {
    await this.settingsClose.click();
    await expect(this.settingsModal).toBeHidden();
  }

  async setProvider(provider: 'local' | 'cloud' | 'all') {
    switch (provider) {
      case 'local':
        await this.providerLocal.click();
        break;
      case 'cloud':
        await this.providerCloud.click();
        break;
      case 'all':
        await this.providerAll.click();
        break;
    }
  }

  async setSystemPrompt(prompt: string) {
    await this.systemPrompt.fill(prompt);
  }

  async sendMessage(message: string) {
    await this.chatInput.fill(message);
    await this.sendButton.click();
  }

  async sendMessageWithEnter(message: string) {
    await this.chatInput.fill(message);
    await this.chatInput.press('Enter');
  }

  async clearChat() {
    await this.openSettings();
    await this.resetChat.click();
    await this.closeSettings();
  }

  async getModelOptions(): Promise<string[]> {
    const options = await this.modelSelect.locator('option').allTextContents();
    return options;
  }

  async getUserMessages(): Promise<Locator> {
    return this.page.locator(PlaygroundSelectors.messages.user);
  }

  async getAssistantMessages(): Promise<Locator> {
    return this.page.locator(PlaygroundSelectors.messages.assistant);
  }

  async dismissError() {
    if (await this.errorBanner.isVisible()) {
      await this.errorClose.click();
    }
  }
}
