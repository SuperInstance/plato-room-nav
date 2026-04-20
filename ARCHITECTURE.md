# Architecture: plato-room-nav

## Language Choice: Rust

### Why Rust

Room navigation is pathfinding — pure BFS/Dijkstra over a small graph.
The case for Rust here is about **service reliability**, not raw speed:
- No GC pauses during navigation queries
- Compact graph representation (rooms stay in L1 cache)
- Can run as a long-lived navigation daemon without memory growth
- WASM-compileable for client-side room maps

### Why not A*

A* needs a heuristic function (Euclidean distance, etc.). Room graphs don't have
spatial coordinates — they're topological. BFS/Dijkstra is optimal for small,
uniform-weight graphs.

### Architecture

```
RoomNav {
    rooms: HashMap<String, Room>
    connections: HashMap<String, Vec<Connection>>
    bookmarks: HashMap<String, Bookmark>
}

navigate(from, to) → Dijkstra → Route { path, weight, hops, directions }
reachable(from, N)  → BFS N-deep → [(room, hops)]
discover(from)      → BFS all    → [rooms]
dead_ends()         → filter connectivity == 1
orphans()           → filter connectivity == 0
```
