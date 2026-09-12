import http from 'node:http';
import net from 'node:net';
import { readFile } from 'node:fs/promises';

const limit = 65536;
const target = /^127\.0\.0\.1:(\d+)$/.exec(process.argv[2] ?? '');
if (!target || Number(target[1]) > 65535 || Number(target[1]) === 0) {
  throw new Error('expected a loopback service address');
}
const assets = new URL('../../target/browser/', import.meta.url);
let origin;
let active = 0;

function exchange(fields, response) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: '127.0.0.1', port: Number(target[1]) });
    let buffered = Buffer.alloc(0);
    const failed = () => reject(new Error('delivery unavailable; retain the receipt'));
    socket.setTimeout(30000, () => socket.destroy(new Error('timeout')));
    socket.on('error', failed);
    socket.on('end', failed);
    socket.on('close', failed);
    response.once('close', () => socket.destroy());
    socket.once('connect', () => socket.write(`v1\t${fields.join('\t')}\n`));
    socket.on('data', chunk => {
      buffered = Buffer.concat([buffered, chunk]);
      if (buffered.length > limit) return socket.destroy(new Error('frame bound'));
      let end;
      while ((end = buffered.indexOf(10)) >= 0) {
        const line = buffered.subarray(0, end).toString('utf8');
        buffered = buffered.subarray(end + 1);
        if (line === 'pending') continue;
        resolve(line);
        socket.destroy();
        return;
      }
    });
  });
}

async function route(request, response) {
  response.setHeader('Cache-Control', 'no-store');
  response.setHeader('X-Content-Type-Options', 'nosniff');
  response.setHeader('Content-Security-Policy', "default-src 'self'; object-src 'none'; frame-ancestors 'none'");
  if (request.headers.host !== new URL(origin).host) {
    response.writeHead(403).end('invalid host'); return;
  }
  if (request.method === 'GET') {
    const files = { '/': [new URL('./index.html', import.meta.url), 'text/html'],
      '/edit': [new URL('./edit.html', import.meta.url), 'text/html'],
      '/edit.js': [new URL('apps/web/edit.js', assets), 'text/javascript'],
      '/editing.js': [new URL('apps/web/editing.js', assets), 'text/javascript'],
      '/modules/text/api/typescript.js': [new URL('modules/text/api/typescript.js', assets), 'text/javascript'],
      '/sdk/typescript/delivery.js': [new URL('sdk/typescript/delivery.js', assets), 'text/javascript'],
      '/app.js': [new URL('apps/web/app.js', assets), 'text/javascript'],
      '/sdk/typescript/resource.js': [new URL('sdk/typescript/resource.js', assets), 'text/javascript'] };
    const file = Object.hasOwn(files, request.url) ? files[request.url] : undefined;
    if (!file) { response.writeHead(404).end(); return; }
    response.setHeader('Content-Type', `${file[1]}; charset=utf-8`);
    response.end(await readFile(file[0])); return;
  }
  if (request.method !== 'POST' || request.url !== '/rpc'
      || request.headers.origin !== origin || request.headers['x-reovim'] !== '1') {
    response.writeHead(403).end('same-origin RPC required'); return;
  }
  if (active >= 16) { response.writeHead(503).end('bridge capacity'); return; }
  active++;
  try {
    const chunks = [];
    let size = 0;
    for await (const chunk of request) {
      size += chunk.length;
      if (size > limit) { response.writeHead(413).end(); return; }
      chunks.push(chunk);
    }
    const data = new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunks));
    const fields = JSON.parse(data);
    if (!Array.isArray(fields) || !fields.length || fields.some(field =>
      typeof field !== 'string' || /[\t\r\n\0]/.test(field))
      || Buffer.byteLength(`v1\t${fields.join('\t')}\n`) > limit) {
      response.writeHead(400).end('invalid frame'); return;
    }
    response.setHeader('Content-Type', 'text/plain; charset=utf-8');
    response.end(await exchange(fields, response));
  } finally { active--; }
}

const server = http.createServer({ requestTimeout: 10000, headersTimeout: 10000,
  maxHeaderSize: 8192 }, (request, response) => {
  route(request, response).catch(() => {
    if (!response.headersSent) response.writeHead(502);
    response.end('delivery unavailable; retain the receipt');
  });
});
server.maxConnections = 32;
server.listen(Number(process.argv[3] ?? 0), '127.0.0.1', () => {
  origin = `http://127.0.0.1:${server.address().port}`;
  console.log(`browser ${origin}`);
});
