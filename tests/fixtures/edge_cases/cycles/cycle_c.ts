// tests/fixtures/edge_cases/cycles/cycle_c.ts
import { stepA } from './cycle_a';

export function stepC(n: number): number {
    if (n <= 0) return 0;
    return stepA(n - 1) + 3;
}
