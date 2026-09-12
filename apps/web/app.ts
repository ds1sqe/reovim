import { Client, message, opening, outcome, type Opening, type Receipt } from '../../sdk/typescript/resource.js';

interface Observation { snapshot: string; watch: string; revision: string;
  snapshotClosed?: boolean; watchClosed?: boolean }
interface Journal { token?: string; opening?: Opening; receipt?: Receipt; observation?: Observation }
const output = document.querySelector('pre')!;
const saved = sessionStorage.getItem('reovim');
const journal: Journal = saved ? JSON.parse(saved) : {};
let busy = false;
const persist = () => sessionStorage.setItem('reovim', JSON.stringify(journal));
function render() {
  const value = journal.observation;
  output.textContent = value ? `Context revision ${value.revision}\nSnapshot ${value.snapshot}\nWatch ${value.watch}`
    : 'No observation is held.';
}
function adopt(fields: string[], line: string) {
  if (fields[2] === 'kernel.session.observe') {
    const value = message(line);
    if (value.contract !== 'kernel.session.observe' || value.resources.length < 3 || value.data.length < 16) {
      throw new Error('invalid observation; receipt retained');
    }
    journal.observation = { snapshot: value.resources[1]!, watch: value.resources[2]!,
      revision: new DataView(value.data.buffer).getBigUint64(0, true).toString() };
  } else if (fields[0] === 'close' && line === 'ok') {
    journal.observation!.watchClosed = true;
  } else if (fields[2] === 'kernel.session.release') {
    const value = message(line);
    if (value.contract !== 'kernel.session.release' || value.data.length || value.resources.length) {
      throw new Error('invalid release');
    }
    journal.observation!.snapshotClosed = true;
  } else { throw new Error(line); }
}
async function execute(client: Client, fields: string[]) {
  if (!journal.receipt) {
    journal.receipt = await client.reserve(fields);
    persist();
  }
  const receipt = journal.receipt;
  receipt.response = await client.send(receipt);
  if (receipt.response.startsWith('error\t')) {
    const result = outcome(receipt.response);
    persist();
    if (result.kind === 'unknown') throw new Error('Unknown outcome; receipt retained');
    await client.release(receipt.id);
    delete journal.receipt; persist();
    throw new Error(receipt.response);
  }
  adopt(receipt.fields, receipt.response);
  persist();
  await client.release(receipt.id);
  delete journal.receipt;
  persist();
}
async function run(release: boolean) {
  if (busy) return;
  busy = true;
  try {
    if (!journal.token && !journal.opening) { journal.opening = opening(); persist(); }
    const client = journal.token ? new Client(journal.token)
      : await Client.connect(journal.opening!, persist);
    journal.token = client.token; persist();
    delete journal.opening; persist();
    if (journal.receipt) await execute(client, journal.receipt.fields);
    if (!release && !journal.observation) {
      await execute(client, ['read', 'session', 'kernel.session.observe', '1']);
    }
    if (release && journal.observation) {
      if (!journal.observation.snapshotClosed) {
        await execute(client, ['write', journal.observation.snapshot, 'kernel.session.release', '1']);
      }
      if (!journal.observation.watchClosed) await execute(client, ['close', journal.observation.watch]);
      delete journal.observation; persist();
    }
    render();
  } catch (error) { output.textContent = String(error); }
  finally { busy = false; }
}
document.querySelector('#connect')!.addEventListener('click', () => { void run(false); });
document.querySelector('#release')!.addEventListener('click', () => { void run(true); });
render();
