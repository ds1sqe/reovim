// The tui in a page: one text Buffer in a textarea, Ctrl-S save, Ctrl-Z
// undo, Ctrl-Y redo, the status line from tree, redrawn on Modify.

import { decodeValue, field } from '../../sdk/typescript/value.js';
import { READ, WRITE } from '../../sdk/typescript/wire.js';
import { Client, Fault, rdInput, rdOutput } from '../../clients/gui/client.js';
import { Ws } from './websocket.js';

const encode = (text: string) => new TextEncoder().encode(text);
const decode = (bytes: Uint8Array) => new TextDecoder().decode(bytes);

const area = document.getElementById('text') as HTMLTextAreaElement;
const status = document.getElementById('status') as HTMLElement;
const file = new URLSearchParams(location.search).get('file') ?? 'note';

async function main(): Promise<void> {
  const carrier = new Ws(`ws://${location.host}/ws`);
  await carrier.connect();
  const client = await Client.connect(carrier, encode(`web:${Date.now()}`));
  await client.capability(encode('web'));
  const source = await client.resolve(`/module/fs/root/${file}`);
  const open = await client.resolve('/module/text/open');
  const [, made] = await client.callByName(open, 'open', rdInput(source));
  const buffer = rdOutput(made);
  const h = await client.open(buffer, READ | WRITE);
  await client.watch(h, true);
  const watched = await client.open(source, READ);
  await client.watch(watched, true);
  await client.since();

  const load = async () => {
    const text = decode(await client.readWhole(h));
    if (area.value !== text) area.value = text;
    const [method, bytes] = await client.callByName(buffer, 'tree', new Uint8Array());
    const tree = decodeValue(method.output, bytes);
    const head = tree && field(tree, 'head');
    const published = tree && field(tree, 'published');
    const mark = head?.kind === 'big' && published?.kind === 'big' && head.value !== published.value ? '[+]' : '';
    status.textContent = `${file}  r${head?.kind === 'big' ? head.value : '?'}${mark}   Ctrl-S save  Ctrl-Z undo  Ctrl-Y redo`;
  };
  await load();

  area.oninput = () => {
    client.write(h, 0n, encode(area.value)).then(load).catch(shown);
  };
  area.onkeydown = (event) => {
    const keys: Record<string, string> = { s: 'save', z: 'undo', y: 'redo' };
    const name = event.ctrlKey ? keys[event.key] : undefined;
    if (name === undefined) return;
    event.preventDefault();
    client.callByName(buffer, name, new Uint8Array()).then(load).catch(shown);
  };
  setInterval(() => {
    client.since().then((events) => {
      const moved = events.some((e) => e.kind === 'overflow'
        || (e.kind === 'modify' && (e.rd === buffer || e.rd === source)));
      return moved ? load() : undefined;
    }).catch(shown);
  }, 100);
}

function shown(error: unknown): void {
  status.textContent = error instanceof Fault ? `reovim: ${error.message}` : String(error);
}

main().catch(shown);
