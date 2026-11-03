import type { WorldTransportOptions } from './WorldTransport'
import { TransportEngine, type SocketFactory } from './transport/engine'
/** Native browser WebSocket; no mock import, subprotocol credential or configurable URL. */
export class WebSocketWorldTransport extends TransportEngine {
  constructor(options: WorldTransportOptions, socketFactory: SocketFactory = url => new WebSocket(url)) {
    super(options, socketFactory)
  }
}
