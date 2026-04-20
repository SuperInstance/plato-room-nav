//! # plato-room-nav
//!
//! Room navigation and pathfinding engine. Finds shortest routes between rooms,
//! discovers connected rooms, and provides spatial awareness for PLATO agents.
//!
//! ## Why Rust
//!
//! Pathfinding is BFS/Dijkstra — CPU-bound, memory-linear in graph size.
//! Rust gives us: stack-allocated queues, no GC during long traversals,
//! and the ability to run navigation as a long-lived service without memory growth.
//!
//! ## Alternatives
//!
//! - **Python (current)**: Fine for small graphs (<100 rooms). GC pauses during
//!   BFS of 10K+ rooms add unpredictable latency.
//!
//! - **A* with heuristics**: Not needed here — room graphs are small and uniform.
//!   BFS/Dijkstra is optimal for unweighted/small-weight graphs.
//!
//! - **Neo4j pathfinding**: Network overhead dominates for <1K rooms.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque, BinaryHeap};
use std::cmp::Ordering;

/// A room in the navigation graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub room_type: RoomType,
    pub capacity: usize,
    pub occupancy: usize,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoomType {
    Hub,
    Corridor,
    Lab,
    Forge,
    Harbor,
    Private,
    Public,
}

/// A connection between rooms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub from: String,
    pub to: String,
    pub weight: f64,
    pub direction: Direction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Direction {
    Bidirectional,
    OneWay,
}

/// A navigation route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub path: Vec<String>,
    pub total_weight: f64,
    pub hops: usize,
    pub directions: Vec<String>,
}

/// Navigation result with route + metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavResult {
    pub route: Option<Route>,
    pub rooms_visited: usize,
    pub alternative_routes: usize,
}

/// Room bookmark for frequently visited rooms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub room_id: String,
    pub label: String,
    pub visit_count: usize,
    pub last_visited: f64,
}

/// The navigation engine.
pub struct RoomNav {
    rooms: HashMap<String, Room>,
    connections: HashMap<String, Vec<Connection>>,
    bookmarks: HashMap<String, Bookmark>,
    visit_history: Vec<(String, f64)>,
}

impl RoomNav {
    pub fn new() -> Self {
        Self { rooms: HashMap::new(), connections: HashMap::new(),
               bookmarks: HashMap::new(), visit_history: Vec::new() }
    }

    /// Add a room.
    pub fn add_room(&mut self, room: Room) {
        let id = room.id.clone();
        self.connections.entry(id.clone()).or_default();
        self.rooms.insert(id, room);
    }

    /// Connect two rooms.
    pub fn connect(&mut self, from: &str, to: &str, weight: f64, direction: Direction) {
        self.connections.entry(from.to_string()).or_default()
            .push(Connection { from: from.to_string(), to: to.to_string(),
                              weight, direction: direction.clone() });
        if direction == Direction::Bidirectional {
            self.connections.entry(to.to_string()).or_default()
                .push(Connection { from: to.to_string(), to: from.to_string(),
                                  weight, direction });
        }
    }

    /// Find shortest path (BFS for unweighted, Dijkstra for weighted).
    pub fn navigate(&self, from: &str, to: &str) -> NavResult {
        if from == to {
            return NavResult { route: Some(Route { path: vec![from.to_string()],
                        total_weight: 0.0, hops: 0, directions: vec![] }),
                        rooms_visited: 1, alternative_routes: 0 };
        }
        let (route, visited) = self.dijkstra(from, to);
        NavResult { route, rooms_visited: visited, alternative_routes: 0 }
    }

    /// Find all rooms reachable within N hops.
    pub fn reachable(&self, from: &str, max_hops: usize) -> Vec<(String, usize)> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        visited.insert(from.to_string());
        queue.push_back((from.to_string(), 0));
        let mut reachable = Vec::new();

        while let Some((current, hops)) = queue.pop_front() {
            if hops > 0 {
                reachable.push((current.clone(), hops));
            }
            if hops >= max_hops { continue; }
            for conn in self.connections.get(&current).unwrap_or(&vec![]) {
                if !visited.contains(&conn.to) {
                    visited.insert(conn.to.clone());
                    queue.push_back((conn.to.clone(), hops + 1));
                }
            }
        }
        reachable.sort_by_key(|(_, h)| *h);
        reachable
    }

    /// Discover all rooms (BFS from a starting room).
    pub fn discover(&self, from: &str) -> Vec<String> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue = VecDeque::new();
        visited.insert(from.to_string());
        queue.push_back(from.to_string());
        while let Some(current) = queue.pop_front() {
            for conn in self.connections.get(&current).unwrap_or(&vec![]) {
                if !visited.contains(&conn.to) {
                    visited.insert(conn.to.clone());
                    queue.push_back(conn.to.clone());
                }
            }
        }
        visited.into_iter().collect()
    }

    /// Find the room hub (most connected room).
    pub fn find_hub(&self) -> Option<String> {
        self.rooms.keys().max_by_key(|id| {
            self.connections.get(*id).map(|c| c.len()).unwrap_or(0)
        }).cloned()
    }

    /// Record a visit to a room.
    pub fn visit(&mut self, room_id: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64()).unwrap_or(0.0);
        self.visit_history.push((room_id.to_string(), now));
        if let Some(bm) = self.bookmarks.get_mut(room_id) {
            bm.visit_count += 1;
            bm.last_visited = now;
        }
    }

    /// Bookmark a room for quick access.
    pub fn bookmark(&mut self, room_id: &str, label: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64()).unwrap_or(0.0);
        self.bookmarks.insert(room_id.to_string(), Bookmark {
            room_id: room_id.to_string(), label: label.to_string(),
            visit_count: 0, last_visited: now,
        });
    }

    /// Get bookmarks sorted by visit count.
    pub fn bookmarks(&self) -> Vec<&Bookmark> {
        let mut bms: Vec<&Bookmark> = self.bookmarks.values().collect();
        bms.sort_by(|a, b| b.visit_count.cmp(&a.visit_count));
        bms
    }

    /// Room info.
    pub fn room(&self, id: &str) -> Option<&Room> {
        self.rooms.get(id)
    }

    /// All rooms of a type.
    pub fn rooms_by_type(&self, room_type: &RoomType) -> Vec<&Room> {
        self.rooms.values().filter(|r| &r.room_type == room_type).collect()
    }

    /// Connection count for a room.
    pub fn connectivity(&self, room_id: &str) -> usize {
        self.connections.get(room_id).map(|c| c.len()).unwrap_or(0)
    }

    /// Dead-end detection: rooms with only one connection.
    pub fn dead_ends(&self) -> Vec<String> {
        self.rooms.keys().filter(|id| self.connectivity(id) <= 1).cloned().collect()
    }

    /// Orphan detection: rooms with zero connections.
    pub fn orphans(&self) -> Vec<String> {
        self.rooms.keys().filter(|id| self.connectivity(id) == 0).cloned().collect()
    }

    fn dijkstra(&self, from: &str, to: &str) -> (Option<Route>, usize) {
        #[derive(PartialEq)]
        struct MinW(f64, String);
        impl Eq for MinW {}
        impl Ord for MinW {
            fn cmp(&self, other: &Self) -> Ordering {
                other.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
            }
        }
        impl PartialOrd for MinW {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
        }

        let mut dist: HashMap<String, f64> = HashMap::new();
        let mut prev: HashMap<String, String> = HashMap::new();
        let mut heap = BinaryHeap::new();
        let mut visited_count = 0;

        dist.insert(from.to_string(), 0.0);
        heap.push(MinW(0.0, from.to_string()));

        while let Some(MinW(d, u)) = heap.pop() {
            visited_count += 1;
            if d > *dist.get(&u).unwrap_or(&f64::INFINITY) { continue; }
            if u == to {
                let mut path = vec![u.clone()];
                let mut directions = Vec::new();
                let mut current = u.clone();
                let total_weight = d;
                while let Some(p) = prev.get(&current) {
                    directions.push(format!("{} → {}", p, current));
                    path.push(p.clone());
                    current = p.clone();
                }
                path.reverse();
                directions.reverse();
                let hops = path.len() - 1;
                return (Some(Route { path, total_weight, hops, directions }),
                        visited_count);
            }
            for conn in self.connections.get(&u).unwrap_or(&vec![]) {
                let new_dist = d + conn.weight;
                if new_dist < *dist.get(&conn.to).unwrap_or(&f64::INFINITY) {
                    dist.insert(conn.to.clone(), new_dist);
                    prev.insert(conn.to.clone(), u.clone());
                    heap.push(MinW(new_dist, conn.to.clone()));
                }
            }
        }
        (None, visited_count)
    }

    pub fn stats(&self) -> NavStats {
        NavStats { rooms: self.rooms.len(), connections: self.connections.values()
                    .map(|v| v.len()).sum::<usize>() / 2,
                    bookmarks: self.bookmarks.len(), visits: self.visit_history.len(),
                    orphans: self.orphans().len(), dead_ends: self.dead_ends().len() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavStats {
    pub rooms: usize,
    pub connections: usize,
    pub bookmarks: usize,
    pub visits: usize,
    pub orphans: usize,
    pub dead_ends: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_nav() -> RoomNav {
        let mut nav = RoomNav::new();
        nav.add_room(Room { id: "hub".into(), name: "Hub".into(), room_type: RoomType::Hub,
                           capacity: 100, occupancy: 5, metadata: HashMap::new() });
        nav.add_room(Room { id: "lab".into(), name: "Lab".into(), room_type: RoomType::Lab,
                           capacity: 10, occupancy: 2, metadata: HashMap::new() });
        nav.add_room(Room { id: "forge".into(), name: "Forge".into(), room_type: RoomType::Forge,
                           capacity: 20, occupancy: 1, metadata: HashMap::new() });
        nav.add_room(Room { id: "harbor".into(), name: "Harbor".into(), room_type: RoomType::Harbor,
                           capacity: 50, occupancy: 3, metadata: HashMap::new() });
        nav.connect("hub", "lab", 1.0, Direction::Bidirectional);
        nav.connect("hub", "forge", 1.0, Direction::Bidirectional);
        nav.connect("hub", "harbor", 2.0, Direction::Bidirectional);
        nav.connect("forge", "harbor", 1.0, Direction::Bidirectional);
        nav
    }

    #[test]
    fn test_navigate() {
        let nav = setup_nav();
        let result = nav.navigate("lab", "harbor");
        assert!(result.route.is_some());
        assert_eq!(result.route.as_ref().unwrap().hops, 2); // lab→hub→harbor
    }

    #[test]
    fn test_reachable() {
        let nav = setup_nav();
        let reachable = nav.reachable("hub", 1);
        assert!(reachable.len() >= 3);
    }

    #[test]
    fn test_discover() {
        let nav = setup_nav();
        let discovered = nav.discover("hub");
        assert_eq!(discovered.len(), 4);
    }

    #[test]
    fn test_dead_ends() {
        let mut nav = RoomNav::new();
        nav.add_room(Room { id: "a".into(), name: "A".into(), room_type: RoomType::Public,
                           capacity: 10, occupancy: 0, metadata: HashMap::new() });
        nav.add_room(Room { id: "b".into(), name: "B".into(), room_type: RoomType::Public,
                           capacity: 10, occupancy: 0, metadata: HashMap::new() });
        nav.connect("a", "b", 1.0, Direction::Bidirectional);
        assert_eq!(nav.dead_ends().len(), 2);
    }

    #[test]
    fn test_bookmark() {
        let mut nav = setup_nav();
        nav.bookmark("hub", "Main Hub");
        nav.visit("hub");
        nav.visit("hub");
        let bms = nav.bookmarks();
        assert_eq!(bms[0].visit_count, 2);
    }
}
