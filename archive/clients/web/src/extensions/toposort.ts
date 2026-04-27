/**
 * Topological sort for extension arrays using Kahn's algorithm (#583).
 *
 * Extensions declare dependencies via optional `dependencies()` method.
 * Missing dependencies (not present in the array) are silently skipped.
 */
export function toposortExtensions<
  T extends { kind(): string; dependencies?(): string[] },
>(extensions: T[]): T[] {
  const byKind = new Map<string, T>();
  const inDegree = new Map<string, number>();
  const graph = new Map<string, string[]>();

  for (const ext of extensions) {
    const kind = ext.kind();
    byKind.set(kind, ext);
    inDegree.set(kind, 0);
    graph.set(kind, []);
  }

  // Build edges: for each declared dependency present in the set,
  // add edge dep -> ext (dep must come before ext).
  for (const ext of extensions) {
    const kind = ext.kind();
    const deps = ext.dependencies?.() ?? [];
    for (const dep of deps) {
      if (byKind.has(dep)) {
        graph.get(dep)!.push(kind);
        inDegree.set(kind, (inDegree.get(kind) ?? 0) + 1);
      }
    }
  }

  // Kahn's: start from in-degree 0 nodes
  const queue: string[] = [];
  for (const [kind, deg] of inDegree) {
    if (deg === 0) queue.push(kind);
  }

  const sorted: T[] = [];
  while (queue.length > 0) {
    const current = queue.shift()!;
    sorted.push(byKind.get(current)!);
    for (const neighbor of graph.get(current) ?? []) {
      const newDeg = (inDegree.get(neighbor) ?? 1) - 1;
      inDegree.set(neighbor, newDeg);
      if (newDeg === 0) queue.push(neighbor);
    }
  }

  if (sorted.length !== extensions.length) {
    throw new Error(
      `Extension dependency cycle detected: resolved ${sorted.length} of ${extensions.length}`,
    );
  }

  return sorted;
}
