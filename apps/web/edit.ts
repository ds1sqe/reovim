import { Editing, type Journal } from './editing.js';
import { unhex } from '../../sdk/typescript/resource.js';

const saved = sessionStorage.getItem('reovim-editing');
const journal: Journal = saved ? JSON.parse(saved) : { retiring: [] };
const editing = new Editing(journal,
  () => sessionStorage.setItem('reovim-editing', JSON.stringify(journal)));
const status = document.querySelector<HTMLElement>('#status')!;
const content = document.querySelector<HTMLElement>('#content')!;
const input = (id: string) => document.querySelector<HTMLInputElement>(`#${id}`)!.value;
let busy = false;

function render() {
  status.textContent = journal.source
    ? `Revision ${journal.source.revision}${journal.receipt ? '; request needs recovery' : ''}`
    : 'No source is open.';
  const value = journal.window;
  content.textContent = value ? value.units.map(unit => 'scalar' in unit
    ? unit.scalar : `⟦${unit.opaque.toString(16).padStart(2, '0')}⟧`).join('') : '';
  document.querySelector('#range')!.textContent = value
    ? `Units ${value.start}–${value.end}${value.eof ? '; end of source' : '; more available'}` : '';
}

const actions: Record<string, () => Promise<void>> = {
  create: () => editing.create(unhex(input('initial').trim())),
  observe: () => editing.observe(),
  read: () => editing.read(BigInt(input('window-start'))),
  replace: () => editing.replace(BigInt(input('start')), BigInt(input('end')), input('replacement')),
  undo: () => editing.undo(), close: () => editing.close(), recover: () => editing.recover()
};
for (const [id, action] of Object.entries(actions)) {
  document.querySelector(`#${id}`)!.addEventListener('click', async () => {
    if (busy) return;
    busy = true;
    try { await action(); render(); }
    catch (error) { render(); status.textContent += `\n${String(error)}`; }
    finally { busy = false; }
  });
}
render();
