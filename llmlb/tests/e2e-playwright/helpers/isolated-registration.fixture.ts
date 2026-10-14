import { spawn, type ChildProcess } from 'node:child_process'
import { once } from 'node:events'
import { access, mkdtemp, rm } from 'node:fs/promises'
import { createServer, type AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import * as path from 'node:path'
import { request as playwrightRequest } from '@playwright/test'
import { test as registrationTest, expect } from './registration.fixture'

async function unusedPort(): Promise<number> {
  const listener = createServer()
  await new Promise<void>((resolve, reject) => {
    listener.once('error', reject)
    listener.listen(0, '127.0.0.1', resolve)
  })
  const port = (listener.address() as AddressInfo).port
  await new Promise<void>((resolve, reject) => listener.close((error) => error ? reject(error) : resolve()))
  return port
}

async function stopOwnedServer(server: ChildProcess): Promise<void> {
  if (!server.pid || server.exitCode !== null || server.signalCode !== null) return
  const stopped = once(server, 'exit', { signal: AbortSignal.timeout(5000) })
  server.kill('SIGTERM')
  try {
    await stopped
  } catch (error) {
    if (!(error instanceof Error) || error.name !== 'AbortError') throw error
    if (server.exitCode !== null || server.signalCode !== null) return
    // Only this fixture's child PID is eligible for termination after the deadline.
    const killed = once(server, 'exit')
    server.kill('SIGKILL')
    await killed
  }
}

/** Last-admin protection needs a database no concurrent suite can populate. */
export const test = registrationTest.extend<{}, { isolatedRegistrationURL: string }>({
  isolatedRegistrationURL: [async ({}, use) => {
    const repoRoot = path.resolve(__dirname, '../../../..')
    const targetDir = process.env.CARGO_TARGET_DIR
      ? path.resolve(repoRoot, process.env.CARGO_TARGET_DIR)
      : path.join(repoRoot, 'target')
    const binary = process.env.LLMLB_E2E_BINARY
      ? path.resolve(repoRoot, process.env.LLMLB_E2E_BINARY)
      : path.join(targetDir, 'debug', process.platform === 'win32' ? 'llmlb.exe' : 'llmlb')
    await access(binary)
    const port = await unusedPort()
    const dataDir = await mkdtemp(path.join(tmpdir(), 'llmlb-registration-e2e-'))
    let output = ''
    let startupError: Error | undefined
    const server = spawn(binary, ['serve', '--no-tray', '--port', String(port)], {
      cwd: repoRoot,
      env: {
        ...process.env,
        TMPDIR: dataDir,
        TMP: dataDir,
        TEMP: dataDir,
        LLMLB_DATABASE_URL: `sqlite:${path.join(dataDir, 'llmlb.db')}`,
        LLMLB_LOG_DIR: path.join(dataDir, 'logs'),
        LLMLB_API_KEY_REQUIRED: 'true',
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    const baseURL = `http://127.0.0.1:${port}`
    server.on('error', (error) => { startupError = error })
    const capture = (chunk: Buffer) => { output = `${output}${chunk.toString()}`.slice(-20000) }
    server.stdout?.on('data', capture)
    server.stderr?.on('data', capture)
    const probe = await playwrightRequest.newContext({ baseURL })
    try {
      await expect.poll(async () => {
        if (startupError) throw startupError
        if (server.exitCode !== null) throw new Error(`isolated llmlb exited (${server.exitCode}): ${output}`)
        return probe.get('/dashboard/login.html').then((response) => response.status()).catch(() => 0)
      }, { timeout: 45000, message: 'isolated llmlb must start with a fresh registration database' }).toBe(200)
      await use(baseURL)
    } finally {
      await probe.dispose()
      await stopOwnedServer(server)
      await rm(dataDir, { recursive: true, force: true })
    }
  }, { scope: 'worker', timeout: 60000 }],
  baseURL: async ({ isolatedRegistrationURL }, use) => use(isolatedRegistrationURL),
})

export { expect }
