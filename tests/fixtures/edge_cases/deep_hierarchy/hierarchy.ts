// tests/fixtures/edge_cases/deep_hierarchy/hierarchy.ts

export class LevelA {
    name: string = "A";
    methodA(): string { return "A"; }
}

export class LevelB extends LevelA {
    methodB(): string { return "B"; }
}

export class LevelC extends LevelB {
    methodC(): string { return "C"; }
}

export class LevelD extends LevelC {
    methodD(): string { return "D"; }
}

export class LevelE extends LevelD {
    methodE(): string { return "E"; }
}
