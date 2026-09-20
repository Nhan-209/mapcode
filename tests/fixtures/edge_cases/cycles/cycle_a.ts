// tests/fixtures/edge_cases/cycles/cycle_a.ts
import { stepB } from './cycle_b';

export function stepA(n: number): number {
    if (n <= 0) return 0;
    return stepB(n - 1) + 1;
}
