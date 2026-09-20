// tests/fixtures/sample_workspace/ts_app/models.ts

export interface BaseRecord {
    id: string;
    createdAt: Date;
}

export interface IOrderService {
    getOrder(id: string): Order | null;
    processOrder(order: Order): boolean;
}

export class Order implements BaseRecord {
    id: string;
    createdAt: Date;
    amount: number;

    constructor(id: string, amount: number) {
        this.id = id;
        this.amount = amount;
        this.createdAt = new Date();
    }

    calculateTax(rate: number): number {
        return this.amount * rate;
    }
}
