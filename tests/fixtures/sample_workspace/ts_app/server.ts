// tests/fixtures/sample_workspace/ts_app/server.ts

import express, { Request, Response } from 'express';
import { Order } from './models';

const app = express();
app.use(express.json());

app.get('/api/v1/orders', (req: Request, res: Response) => {
    const sample = new Order('ord_100', 49.99);
    res.json([sample]);
});

app.post('/api/v1/orders', (req: Request, res: Response) => {
    const { amount } = req.body;
    const newOrder = new Order('ord_' + Date.now(), amount || 0);
    res.status(201).json(newOrder);
});

const PORT = process.env.PORT || 3000;
app.listen(PORT, () => {
    console.log(`Server listening on port ${PORT}`);
});
