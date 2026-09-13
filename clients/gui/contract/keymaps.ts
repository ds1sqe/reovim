import type { Registration } from './commands.js';

/** Logical key, separate from committed text and device scan codes. */
export interface Key {
  key: string; ctrl?: boolean; alt?: boolean; shift?: boolean; meta?: boolean;
}
export interface Binding {
  sequence: readonly Key[];
  /** Empty scope is the family default; active named scopes precede it. */
  scope: string;
  priority: number;
  command: string;
}
export type RegisterKeymap = (binding: Binding) => Registration;
