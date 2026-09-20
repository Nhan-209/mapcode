// tests/fixtures/sample_workspace/ts_app/events.ts

import { EventEmitter } from 'events';
import { Order } from './models';

export const orderEvents = new EventEmitter();

orderEvents.on('orderCreated', (order: Order) => {
    console.log(`[Event] Order created with id ${order.id}`);
});

orderEvents.on('orderCancelled', (orderId: string) => {
    console.log(`[Event] Order cancelled: ${orderId}`);
});
