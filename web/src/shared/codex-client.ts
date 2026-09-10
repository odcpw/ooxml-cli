import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { createInterface } from 'node:readline';

// Protocol pinned to @openai/codex 0.154.0 (including dynamic tools).
export class CodexClient {
  private child: ChildProcessWithoutNullStreams;
  private sequence = 0;
  private pending = new Map<number, { resolve: (value: any) => void; reject: (error: Error) => void; timer: NodeJS.Timeout }>();
  onNotification: (method: string, params: any) => void = () => {};
  onRequest: (method: string, params: any) => Promise<any> = async () => { throw Error('Unsupported worker request'); };
  onExit: (error: Error) => void = () => {};

  constructor(home: string, cwd: string) {
    this.child = spawn(process.env.OOXML_CODEX_BIN || 'codex', ['app-server', '--listen', 'stdio://'], {
      cwd, env: { PATH: process.env.PATH, HOME: home, CODEX_HOME: home, RUST_LOG: 'error' },
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    // Never log raw worker stderr: it may contain private document text.
    this.child.stderr.resume();
    const fail = (error: Error) => {
      for (const p of this.pending.values()) { clearTimeout(p.timer); p.reject(error); }
      this.pending.clear(); this.onExit(error);
    };
    this.child.on('error', fail);
    this.child.on('exit', code => fail(Error(`Codex worker exited (${code ?? 'signal'}). Saved edits are retained.`)));
    createInterface({ input: this.child.stdout }).on('line', line => {
      let message: any;
      try { message = JSON.parse(line); } catch { return; }
      if (message.method && message.id !== undefined) {
        void this.onRequest(message.method, message.params).then(
          result => this.send({ id: message.id, result }),
          error => this.send({ id: message.id, error: { code: -32603, message: String(error.message || error) } }),
        );
      } else if (message.method) this.onNotification(message.method, message.params);
      else {
        const p = this.pending.get(message.id); if (!p) return;
        this.pending.delete(message.id); clearTimeout(p.timer);
        if (message.error) p.reject(Error(message.error.message)); else p.resolve(message.result);
      }
    });
  }
  private send(message: unknown) { if (!this.child.stdin.destroyed) this.child.stdin.write(JSON.stringify(message) + '\n'); }
  request(method: string, params: unknown): Promise<any> {
    const id = ++this.sequence;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(Error(`Codex ${method} did not respond.`)); }, 60_000);
      this.pending.set(id, { resolve, reject, timer }); this.send({ id, method, params });
    });
  }
  async initialize() {
    await this.request('initialize', { clientInfo: { name: 'ooxml_workbench', version: '1.0.0' }, capabilities: { experimentalApi: true } });
    this.send({ method: 'initialized', params: {} });
    if (!process.env.OPENAI_API_KEY) throw Error('The server API key is not configured.');
    await this.request('account/login/start', { type: 'apiKey', apiKey: process.env.OPENAI_API_KEY });
  }
  close() { this.child.kill('SIGTERM'); }
}
