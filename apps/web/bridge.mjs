// The bridge: a browser speaks WebSocket, reovim serves a Unix socket.
// One process serves the page and moves whole messages between the
// two, one link per WebSocket. No package: the handshake and the
// framing are written out.
//
//   node apps/web/bridge.mjs [--socket PATH] [--port N]

import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import net from 'node:net';
import { fileURLToPath } from 'node:url';

const GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';
const ROOT = new URL('../../', import.meta.url);
const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript' };

/** `/` is the page, `/js/*` the built modules, `/ws` the link. */
async function page(req, res) {
  const url = new URL(req.url, 'http://x');
  const path = url.pathname === '/' ? 'apps/web/edit.html'
    : url.pathname.startsWith('/js/') ? `target/browser/${url.pathname.slice(4)}`
    : undefined;
  const type = path && TYPES[path.slice(path.lastIndexOf('.'))];
  if (path === undefined || !type || path.includes('..')) {
    res.writeHead(404).end();
    return;
  }
  try {
    const body = await readFile(new URL(path, ROOT));
    res.writeHead(200, { 'content-type': type }).end(body);
  } catch {
    res.writeHead(404).end();
  }
}

/** One WebSocket and its link, frames to messages and back. */
class Bridge {
  constructor(ws, socket) {
    this.ws = ws;
    this.fromWs = Buffer.alloc(0);
    this.fromLink = Buffer.alloc(0);
    this.link = net.createConnection(socket);
    ws.on('data', (data) => this.browser(data));
    this.link.on('data', (data) => this.server(data));
    for (const end of ['close', 'error']) {
      ws.on(end, () => this.link.destroy());
      this.link.on(end, () => ws.destroy());
    }
  }

  /** WebSocket frames in; a binary message is one link message. */
  browser(data) {
    this.fromWs = Buffer.concat([this.fromWs, data]);
    for (;;) {
      const frame = parseFrame(this.fromWs);
      if (frame === undefined) return;
      this.fromWs = this.fromWs.subarray(frame.size);
      if (frame.opcode === 8) {
        this.ws.end(makeFrame(8, Buffer.alloc(0)));
        return;
      }
      if (frame.opcode === 9) this.ws.write(makeFrame(10, frame.payload));
      if (frame.opcode !== 1 && frame.opcode !== 2) continue;
      const head = Buffer.alloc(4);
      head.writeUInt32LE(frame.payload.length, 0);
      this.link.write(Buffer.concat([head, frame.payload]));
    }
  }

  /** Link messages in; each one is a binary frame out. */
  server(data) {
    this.fromLink = Buffer.concat([this.fromLink, data]);
    while (this.fromLink.length >= 4) {
      const len = this.fromLink.readUInt32LE(0);
      if (this.fromLink.length < 4 + len) return;
      this.ws.write(makeFrame(2, this.fromLink.subarray(4, 4 + len)));
      this.fromLink = this.fromLink.subarray(4 + len);
    }
  }
}

/** One client frame from the front of `buf`, if whole. */
function parseFrame(buf) {
  if (buf.length < 2) return undefined;
  const opcode = buf[0] & 0x0f;
  const masked = (buf[1] & 0x80) !== 0;
  let len = buf[1] & 0x7f;
  let at = 2;
  if (len === 126) {
    if (buf.length < 4) return undefined;
    len = buf.readUInt16BE(2);
    at = 4;
  } else if (len === 127) {
    if (buf.length < 10) return undefined;
    len = Number(buf.readBigUInt64BE(2));
    at = 10;
  }
  const size = at + (masked ? 4 : 0) + len;
  if (buf.length < size) return undefined;
  const payload = Buffer.from(buf.subarray(at + (masked ? 4 : 0), size));
  if (masked) {
    const key = buf.subarray(at, at + 4);
    for (let i = 0; i < payload.length; i += 1) payload[i] ^= key[i % 4];
  }
  return { opcode, payload, size };
}

/** One unmasked server frame. */
function makeFrame(opcode, payload) {
  const len = payload.length;
  const head = len < 126 ? Buffer.from([0x80 | opcode, len])
    : len < 65536 ? Buffer.from([0x80 | opcode, 126, len >> 8, len & 0xff])
    : Buffer.concat([Buffer.from([0x80 | opcode, 127]), (() => { const b = Buffer.alloc(8); b.writeBigUInt64BE(BigInt(len)); return b; })()]);
  return Buffer.concat([head, payload]);
}

/** Serve on `port` (0 for any), links to `socket`. */
export function serve({ socket, port }) {
  const server = createServer(page);
  server.on('upgrade', (req, ws) => {
    const key = req.headers['sec-websocket-key'];
    if (new URL(req.url, 'http://x').pathname !== '/ws' || key === undefined) {
      ws.destroy();
      return;
    }
    const accept = createHash('sha1').update(key + GUID).digest('base64');
    ws.write('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n'
      + `Connection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    new Bridge(ws, socket);
  });
  server.listen(port);
  return server;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const arg = (name, fallback) => {
    const at = process.argv.indexOf(name);
    return at > 0 ? process.argv[at + 1] : fallback;
  };
  const socket = arg('--socket', `${process.env.XDG_RUNTIME_DIR ?? '/tmp'}/reovim.sock`);
  const server = serve({ socket, port: Number(arg('--port', '8790')) });
  server.on('listening', () => {
    console.log(`reovim bridge: http://127.0.0.1:${server.address().port}/?file=NAME -> ${socket}`);
  });
}
