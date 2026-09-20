// tests/fixtures/edge_cases/cycles/cycle_b.ts
import { stepC } from './cycle_c';

export function stepB(n: number): number {
    if (n <= 0) return 0;
    return stepC(n - 1) + 2;
}
