// A Carrier for Node over the server's Unix socket: whole messages
// framed with a u32 length, as host/os frames them.

import net from 'node:net';

import { Fault } from '../../target/browser/clients/gui/client.js';

export class Socket {
  constructor(path) {
    this.path = path;
    this.socket = undefined;
    this.pending = Buffer.alloc(0);
    this.waiting = [];
  }

  connect() {
    return new Promise((resolve, reject) => {
      const socket = net.createConnection(this.path);
      socket.on('connect', () => {
        this.socket = socket;
        resolve();
      });
      socket.on('error', (error) => {
        if (this.socket !== socket) reject(error);
        this.fail();
      });
      socket.on('close', () => this.fail());
      socket.on('data', (data) => this.arrived(data));
    });
  }

  fail() {
    this.socket = undefined;
    for (const waiter of this.waiting.splice(0)) waiter.reject(new Fault('carrier'));
  }

  arrived(data) {
    this.pending = Buffer.concat([this.pending, data]);
    while (this.pending.length >= 4) {
      const len = this.pending.readUInt32LE(0);
      if (this.pending.length < 4 + len) return;
      const frame = Uint8Array.from(this.pending.subarray(4, 4 + len));
      this.pending = this.pending.subarray(4 + len);
      this.waiting.shift()?.resolve(frame);
    }
  }

  exchange(bytes) {
    if (this.socket === undefined) return Promise.reject(new Fault('carrier'));
    const head = Buffer.alloc(4);
    head.writeUInt32LE(bytes.length, 0);
    this.socket.write(Buffer.concat([head, Buffer.from(bytes)]));
    return new Promise((resolve, reject) => this.waiting.push({ resolve, reject }));
  }

  async reconnect() {
    this.pending = Buffer.alloc(0);
    try {
      await this.connect();
    } catch {
      throw new Fault('carrier');
    }
  }

  end() {
    this.socket?.destroy();
  }
}
