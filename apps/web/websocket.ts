// The browser's Carrier: one binary message out, the next one in,
// through the bridge; a fresh WebSocket when the old one closed.

import { type Carrier, Fault } from '../../clients/gui/client.js';

interface Waiter {
  resolve(bytes: Uint8Array): void;
  reject(fault: Fault): void;
}

export class Ws implements Carrier {
  private ws?: WebSocket;
  private waiting: Waiter[] = [];

  constructor(private url: string) {}

  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(this.url);
      ws.binaryType = 'arraybuffer';
      ws.onopen = () => {
        this.ws = ws;
        resolve();
      };
      ws.onerror = () => {
        if (this.ws !== ws) reject(new Fault('carrier'));
        this.fail();
      };
      ws.onclose = () => this.fail();
      ws.onmessage = (event) => {
        this.waiting.shift()?.resolve(new Uint8Array(event.data as ArrayBuffer));
      };
    });
  }

  private fail(): void {
    this.ws = undefined;
    for (const waiter of this.waiting.splice(0)) waiter.reject(new Fault('carrier'));
  }

  exchange(bytes: Uint8Array): Promise<Uint8Array> {
    if (this.ws === undefined) return Promise.reject(new Fault('carrier'));
    this.ws.send(bytes.slice());
    return new Promise((resolve, reject) => this.waiting.push({ resolve, reject }));
  }

  async reconnect(): Promise<void> {
    await this.connect();
  }
}
