import { Editing, type Journal } from './editing.js';
import { Client, unhex } from '../../sdk/typescript/resource.js';
import { Focus, type ContextJournal } from '../../clients/gui/runtime/context.js';
import { Switching, type SwitchJournal } from '../../clients/gui/runtime/switching.js';
import { input as bindInput, type Draft } from './input.js';
import { Commands } from '../../clients/gui/runtime/commands.js';
import { editor as editorCapability } from '../../clients/gui/runtime/editor.js';
import type { Registration } from '../../clients/gui/contract/commands.js';
import { selectedRange } from '../../clients/gui/contract/input.js';

const saved = sessionStorage.getItem('reovim-editing');
const journal: Journal & { draft?: Draft; focus?: ContextJournal; switching?: SwitchJournal } = saved ? JSON.parse(saved) : { retiring: [] };
const persist = () => sessionStorage.setItem('reovim-editing', JSON.stringify(journal));
const editing = new Editing(journal,
  persist);
const status = document.querySelector<HTMLElement>('#status')!;
const content = document.querySelector<HTMLElement>('#content')!;
const input = (id: string) => document.querySelector<HTMLInputElement>(`#${id}`)!.value;
let busy = false;
let changingView = false;
const textarea = document.querySelector<HTMLTextAreaElement>('#typing')!;
const typing = bindInput(textarea,
  (start, end, text) => operate(() => editing.replace(start, end, text,
    receipt => typing.correlate(receipt.client, receipt.id))),
  direction => operate(() => editing.history(direction)), report, {
    load: () => journal.draft,
    save: value => { journal.draft = value; persist(); draftStatus(); }
  });
const commands = new Commands();
const moduleEditor = editorCapability(() => {
  if (busy || typing.composing || typing.retained || journal.receipt || journal.switching?.pending) {
    throw new Error('Finish pending input before a contribution command');
  }
  if (!journal.observed || !journal.window) throw new Error('Observe Text first');
  return { snapshot: journal.observed.snapshot, window: journal.window,
    ...selectedRange(journal.window, textarea.value, textarea.selectionStart, textarea.selectionEnd) };
}, (start, end, text) => operate(() => editing.replace(start, end, text)));
let contribution: Registration | undefined;
let loading = false;
let context: Focus | undefined;
function focus(): Focus {
  if (!journal.token) throw new Error('Connect to a source first');
  journal.focus ??= { retiring: [] };
  context ??= new Focus(new Client(journal.token), journal.focus, persist);
  return context;
}

function knownSources() {
  return [...(journal.inactive ?? []).map(source => source.buffer),
    ...(journal.source ? [journal.source.buffer] : [])];
}
let selector: Switching | undefined;
function switching(): Switching {
  journal.switching ??= {};
  selector ??= new Switching(focus(), { known: knownSources, current: () => journal.source?.buffer,
    pending: () => !!(journal.receipt || journal.resuming || journal.openingCleanup
      || journal.openingWork?.length || journal.retiring.length),
    show: target => target ? editing.switchTo(target) : editing.park(),
    recover: () => editing.recover(),
    close: async target => {
      if (!knownSources().includes(target)) return;
      if (journal.source?.buffer !== target) throw new Error('Recover the original close target first');
      await editing.close();
    } }, journal.switching, persist);
  return selector;
}

function draftStatus() {
  document.querySelector('#draft-status')!.textContent = journal.draft
    ? `Retained ${journal.draft.phase} draft. Recover pending work before further input.` : '';
}

async function operate<T>(action: () => Promise<T>): Promise<T> {
  if (busy) throw new Error('Wait for the pending operation');
  busy = true;
  try { const result = await action(); render(); return result; }
  finally { busy = false; }
}
function report(error: unknown) {
  render(); status.textContent += `\n${String(error)}`;
}

function render() {
  typing.suspend(changingView || !!journal.switching?.pending);
  const sources = document.querySelector<HTMLSelectElement>('#open-sources')!;
  const selected = sources.value;
  sources.replaceChildren(...(journal.inactive ?? []).map(source => {
    const option = document.createElement('option');
    option.value = source.buffer; option.textContent = source.name ?? 'Shared source';
    return option;
  }));
  if ([...sources.options].some(option => option.value === selected)) sources.value = selected;
  typing.update(journal.receipt ? undefined : journal.window);
  status.textContent = journal.source
    ? `${journal.source.name ?? 'Shared source'}: Revision ${journal.source.revision}${journal.receipt || journal.resuming || journal.openingCleanup ? '; request needs recovery' : ''}`
    : 'No source is open.';
  draftStatus();
  if (journal.switching?.pending) status.textContent += '\nContext switch needs recovery.';
  const current = journal.focus?.current;
  document.querySelector('#focus-status')!.textContent = current
    ? `${journal.focus?.ownership ?? 'private'} context (${journal.focus?.historical ? 'historical' : 'live'}): ${current.active ?? 'no active Buffer'}`
    : 'No context.';
  document.querySelector('#focus-change')!.textContent = journal.focus?.change
    ? `Context delivery: ${journal.focus.change.kind}` : '';
  document.querySelector('#context-checkpoint')!.textContent = journal.focus?.checkpoint
    ? `Checkpoint retained: ${journal.focus.checkpoint.active ?? 'no active Buffer'}` : 'No context checkpoint.';
  const value = journal.window;
  content.textContent = value ? value.units.map(unit => 'scalar' in unit
    ? unit.scalar : `⟦${unit.opaque.toString(16).padStart(2, '0')}⟧`).join('') : '';
  document.querySelector('#range')!.textContent = value
    ? `Units ${value.start}–${value.end}${value.eof ? '; end of source' : '; more available'}` : '';
  const delivery = journal.change?.delivery;
  document.querySelector('#changes')!.textContent = !delivery ? ''
    : delivery.kind === 'item' ? `${delivery.transition}: ${delivery.before} → ${delivery.after}`
    : delivery.kind === 'gap' ? `Change gap${journal.needsRefresh ? '; refresh required' : '; observation renewed'}`
    : delivery.kind === 'end' ? `Watch ended${delivery.lost ? '; changes may be missing' : ''}`
    : 'No pending changes.';
}

const actions: Record<string, () => Promise<unknown>> = {
  create: () => editing.create(unhex(input('initial').trim())),
  'open-file': () => editing.openFile(input('filename')),
  join: () => editing.join(),
  'park-source': () => journal.focus?.current ? switching().select() : editing.park(),
  'switch-source': () => {
    const value = document.querySelector<HTMLSelectElement>('#open-sources')!.value;
    const source = journal.inactive?.find(source => source.buffer === value);
    if (!source) throw new Error('Choose an open source first');
    return journal.focus?.current ? switching().select(source.buffer) : editing.switchTo(source.buffer);
  },
  observe: () => editing.observe(),
  follow: () => editing.follow(),
  read: () => editing.read(BigInt(input('window-start'))),
  replace: () => editing.replace(BigInt(input('start')), BigInt(input('end')), input('replacement')),
  close: () => journal.focus?.current && journal.source
    ? switching().close(journal.source.buffer) : editing.close(),
  recover: () => journal.switching?.pending ? switching().recover() : editing.recover(),
  'create-context': () => focus().create({ known: knownSources(), active: journal.source?.buffer }),
  'join-context': () => focus().join(),
  'focus-source': () => {
    if (!journal.source) throw new Error('Open a source first');
    return switching().select(journal.source.buffer);
  },
  'clear-focus': () => focus().set({ known: journal.focus?.current?.known ?? [] }),
  'refresh-context': () => focus().refresh(),
  'follow-context': () => focus().follow(),
  'remember-context': () => focus().remember(),
  'seek-context': () => focus().seek(),
  'close-context': () => focus().close(),
  'recover-context': () => journal.switching?.pending ? switching().recover() : focus().recover()
};
for (const [id, action] of Object.entries(actions)) {
  document.querySelector(`#${id}`)!.addEventListener('click', async () => {
    if (busy) { status.textContent = 'Wait for the pending operation'; return; }
    if (journal.focus?.receipt && ['create', 'open-file', 'join', 'park-source', 'switch-source', 'close'].includes(id)) {
      status.textContent = 'Recover context work first.'; return;
    }
    const replacingView = journal.switching?.pending?.phase === 'view'
      && !journal.switching.pending.closing
      && ['park-source', 'switch-source'].includes(id);
    if (journal.switching?.pending && !replacingView && !['recover', 'recover-context'].includes(id)) {
      status.textContent = 'Recover the context switch first.'; return;
    }
    if (typing.composing) { status.textContent = 'Finish composition first.'; return; }
    if (typing.retained
        && ['create', 'open-file', 'join', 'park-source', 'switch-source', 'focus-source', 'replace', 'undo', 'close'].includes(id)) {
      status.textContent = 'Resolve or discard the retained local draft first.'; return;
    }
    changingView = ['create', 'open-file', 'join', 'park-source', 'switch-source', 'focus-source', 'recover', 'recover-context', 'close'].includes(id);
    render();
    try { await operate(action); }
    catch (error) { report(error); }
    finally { changingView = false; typing.suspend(!!journal.switching?.pending); }
  });
}
for (const direction of ['undo', 'redo'] as const) {
  document.querySelector(`#${direction}`)!.addEventListener('click', () => {
    void typing.history(direction).catch(report);
  });
}
document.querySelector('#discard-draft')!.addEventListener('click', () => {
  try {
    if (busy) throw new Error('Finish pending work first');
    typing.discard(); render();
  } catch (error) { report(error); }
});
document.querySelector('#resume-input')!.addEventListener('click', async () => {
  try {
    const request = typing.resumeRequest;
    const actual = await operate(() => editing.inputCheckpoint(request.client, request.id, request.after));
    typing.resume(request.client, request.id, actual);
    render();
  } catch (error) { report(error); }
});
document.querySelector('#load-contribution')!.addEventListener('click', async () => {
  try {
    if (loading || contribution) throw new Error('contribution already loading or loaded');
    loading = true;
    const module = await import('../../clients/gui/modules/uppercase/index.js');
    contribution = module.activate(moduleEditor, commands.register);
    document.querySelector('#contribution-status')!.textContent = 'Uppercase contribution loaded.';
  } catch (error) { report(error); }
  finally { loading = false; }
});
document.querySelector('#uppercase')!.addEventListener('click', () => {
  void commands.invoke('text.uppercase').catch(report);
});
document.querySelector('#unload-contribution')!.addEventListener('click', () => {
  try {
    if (loading) throw new Error('contribution loading; retry unload');
    if (!contribution) throw new Error('contribution is not loaded');
    contribution.close(); contribution = undefined;
    document.querySelector('#contribution-status')!.textContent = 'Uppercase contribution unloaded.';
  } catch (error) { report(error); }
});
render();
