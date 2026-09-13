import type { Binding, Key } from '../contract/keymaps.js';
import type { Registration } from '../contract/commands.js';

export interface Target { available(): boolean; invoke(): Promise<void> }
export type Resolution = { kind: 'unhandled' | 'ambiguous' }
  | { kind: 'pending'; deadline: number }
  | { kind: 'command'; target: Target };
interface Entry { binding: Binding; keys: string[]; target: Target }
const CHORD_LIMIT = 4, BINDING_LIMIT = 64, WAIT_MS = 750;

function token(key: Key): string {
  if (!key.key || key.key.length > 32) throw new Error('invalid logical key');
  return JSON.stringify([key.key.toLowerCase(), !!key.ctrl, !!key.alt, !!key.shift, !!key.meta]);
}

/** Pure resolution mechanics. The adapter owns the clock, context and dispatch. */
export class Keymaps {
  private readonly entries = new Set<Entry>();
  private pending: string[] = [];
  private context = '';
  private deadline = 0;
  private lastTime = 0;
  private owners: Target[] = [];

  register(binding: Binding, target: Target): Registration {
    if (!binding.sequence.length || binding.sequence.length > CHORD_LIMIT
      || !Number.isSafeInteger(binding.priority) || binding.scope.length > 64) {
      throw new Error('invalid key binding');
    }
    if (!target.available()) throw new Error('command unavailable');
    if (this.entries.size === BINDING_LIMIT) throw new Error('key binding capacity');
    const keys = binding.sequence.map(token);
    for (const entry of this.entries) {
      if (entry.binding.scope === binding.scope && entry.binding.priority === binding.priority
        && JSON.stringify(entry.keys) === JSON.stringify(keys)) throw new Error('key binding conflict');
    }
    const entry = { binding: { ...binding, sequence: [] }, keys, target };
    this.entries.add(entry); this.clear();
    return { close: () => { if (this.entries.delete(entry)) this.clear(); } };
  }

  clear(): void { this.pending = []; this.deadline = 0; this.owners = []; }

  private prepare(scopes: readonly string[], now: number): void {
    if (!Number.isFinite(now) || now < this.lastTime) throw new Error('non-monotonic key clock');
    this.lastTime = now;
    if (this.owners.some(owner => !owner.available())) this.clear();
    const context = JSON.stringify([...new Set(scopes)].sort());
    if (context !== this.context) { this.clear(); this.context = context; }
  }

  private candidates(keys: readonly string[], scopes: readonly string[]): Entry[] {
    return [...this.entries].filter(entry => entry.target.available()
      && (!entry.binding.scope || scopes.includes(entry.binding.scope))
      && keys.every((key, index) => entry.keys[index] === key));
  }

  private choose(entries: Entry[]): Resolution {
    if (!entries.length) return { kind: 'unhandled' };
    const specific = entries.some(entry => !!entry.binding.scope);
    const scoped = entries.filter(entry => !!entry.binding.scope === specific);
    const priority = Math.max(...scoped.map(entry => entry.binding.priority));
    const best = scoped.filter(entry => entry.binding.priority === priority);
    return best.length === 1 ? { kind: 'command', target: best[0]!.target }
      : { kind: 'ambiguous' };
  }

  expire(scopes: readonly string[], now: number): Resolution {
    this.prepare(scopes, now);
    if (!this.pending.length) return { kind: 'unhandled' };
    if (now < this.deadline) return { kind: 'pending', deadline: this.deadline };
    const matches = this.candidates(this.pending, scopes)
      .filter(entry => entry.keys.length === this.pending.length);
    this.clear();
    return this.choose(matches);
  }

  press(key: Key, scopes: readonly string[], now: number): Resolution {
    this.prepare(scopes, now);
    // The adapter expires due chords before delivering another key.
    if (this.pending.length && now >= this.deadline) throw new Error('expire pending chord first');
    const next = token(key);
    let keys = [...this.pending, next];
    let matches = this.candidates(keys, scopes);
    if (!matches.length && this.pending.length) {
      this.clear(); keys = [next]; matches = this.candidates(keys, scopes);
    }
    if (matches.some(entry => entry.keys.length > keys.length)) {
      this.pending = keys; this.deadline = now + WAIT_MS;
      this.owners = matches.map(entry => entry.target);
      return { kind: 'pending', deadline: this.deadline };
    }
    this.clear();
    return this.choose(matches);
  }
}
