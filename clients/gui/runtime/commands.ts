import type { RegisterCommand } from '../contract/commands.js';

interface Command { run(): Promise<void>; active: number; retiring: boolean }

/** Owns command callback lifetime; it supplies no Kernel or document authority. */
export class Commands {
  private readonly entries = new Map<string, Command>();
  readonly register: RegisterCommand = (name, run) => {
    if (!/^[a-z][a-z0-9._-]{0,63}$/.test(name)) throw new Error('invalid command name');
    if (this.entries.has(name)) throw new Error('command already registered');
    if (this.entries.size === 64) throw new Error('command registration capacity');
    const command = { run, active: 0, retiring: false };
    this.entries.set(name, command);
    return { close: () => {
      command.retiring = true;
      if (command.active) throw new Error('command busy; retry unload');
      if (this.entries.get(name) === command) this.entries.delete(name);
    } };
  };

  async invoke(name: string): Promise<void> {
    const command = this.entries.get(name);
    if (!command || command.retiring) throw new Error('command unavailable');
    if (command.active === 32) throw new Error('command invocation capacity');
    command.active++;
    try { await command.run(); }
    finally { command.active--; }
  }
}
