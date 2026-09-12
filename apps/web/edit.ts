import { Editing, type Journal } from './editing.js';
import { unhex } from '../../sdk/typescript/resource.js';
import { input as bindInput } from './input.js';

const saved = sessionStorage.getItem('reovim-editing');
const journal: Journal = saved ? JSON.parse(saved) : { retiring: [] };
const editing = new Editing(journal,
  () => sessionStorage.setItem('reovim-editing', JSON.stringify(journal)));
const status = document.querySelector<HTMLElement>('#status')!;
const content = document.querySelector<HTMLElement>('#content')!;
const input = (id: string) => document.querySelector<HTMLInputElement>(`#${id}`)!.value;
let busy = false;
const typing = bindInput(document.querySelector<HTMLTextAreaElement>('#typing')!,
  (start, end, text) => operate(() => editing.replace(start, end, text)),
  () => operate(() => editing.undo()), report);

async function operate(action: () => Promise<void>) {
  if (busy) throw new Error('Wait for the pending operation');
  busy = true;
  try { await action(); render(); }
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

const actions: Record<string, () => Promise<void>> = {
  create: () => editing.create(unhex(input('initial').trim())),
  join: () => editing.join(),
  observe: () => editing.observe(),
  follow: () => editing.follow(),
  read: () => editing.read(BigInt(input('window-start'))),
  replace: () => editing.replace(BigInt(input('start')), BigInt(input('end')), input('replacement')),
  undo: () => editing.undo(), close: () => editing.close(), recover: () => editing.recover()
};
for (const [id, action] of Object.entries(actions)) {
  document.querySelector(`#${id}`)!.addEventListener('click', async () => {
    if (typing.composing) { status.textContent = 'Finish composition first.'; return; }
    try { await operate(action); }
    catch (error) { report(error); }
  });
}
render();
