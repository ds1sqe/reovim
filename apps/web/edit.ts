import { Editing, type Journal } from './editing.js';
import { unhex } from '../../sdk/typescript/resource.js';
import { input as bindInput, type Draft } from './input.js';
import { Commands } from '../../clients/gui/runtime/commands.js';
import { editor as editorCapability } from '../../clients/gui/runtime/editor.js';
import type { Registration } from '../../clients/gui/contract/commands.js';
import { selectedRange } from '../../clients/gui/contract/input.js';

const saved = sessionStorage.getItem('reovim-editing');
const journal: Journal & { draft?: Draft } = saved ? JSON.parse(saved) : { retiring: [] };
const persist = () => sessionStorage.setItem('reovim-editing', JSON.stringify(journal));
const editing = new Editing(journal,
  persist);
const status = document.querySelector<HTMLElement>('#status')!;
const content = document.querySelector<HTMLElement>('#content')!;
const input = (id: string) => document.querySelector<HTMLInputElement>(`#${id}`)!.value;
let busy = false;
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
  if (busy || typing.composing || typing.retained || journal.receipt) {
    throw new Error('Finish pending input before a contribution command');
  }
  if (!journal.observed || !journal.window) throw new Error('Observe Text first');
  return { snapshot: journal.observed.snapshot, window: journal.window,
    ...selectedRange(journal.window, textarea.value, textarea.selectionStart, textarea.selectionEnd) };
}, (start, end, text) => operate(() => editing.replace(start, end, text)));
let contribution: Registration | undefined;
let loading = false;

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
  typing.update(journal.receipt ? undefined : journal.window);
  status.textContent = journal.source
    ? `Revision ${journal.source.revision}${journal.receipt ? '; request needs recovery' : ''}`
    : 'No source is open.';
  draftStatus();
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
  join: () => editing.join(),
  observe: () => editing.observe(),
  follow: () => editing.follow(),
  read: () => editing.read(BigInt(input('window-start'))),
  replace: () => editing.replace(BigInt(input('start')), BigInt(input('end')), input('replacement')),
  close: () => editing.close(), recover: () => editing.recover()
};
for (const [id, action] of Object.entries(actions)) {
  document.querySelector(`#${id}`)!.addEventListener('click', async () => {
    if (typing.composing) { status.textContent = 'Finish composition first.'; return; }
    if (typing.retained && ['create', 'join', 'replace', 'undo', 'close'].includes(id)) {
      status.textContent = 'Resolve or discard the retained local draft first.'; return;
    }
    try { await operate(action); }
    catch (error) { report(error); }
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
