use std::collections::{HashMap, HashSet};

pub struct GalleryState {
    pub in_gallery: HashSet<String>,
    pub in_rooms: HashMap<String, u64>,
}

impl GalleryState {
    pub fn new() -> Self {
        Self {
            in_gallery: HashSet::new(),
            in_rooms: HashMap::new(),
        }
    }

    pub fn apply_entry(&mut self, entry: &str, name: &str, room: Option<u64>) -> Result<(), ()> {
        match entry {
            "A" => {
                if let Some(r) = room {
                    if !self.in_gallery.contains(name) {
                        return Err(());
                    }
                    if self.in_rooms.contains_key(name) {
                        return Err(());
                    }
                    self.in_rooms.insert(name.to_string(), r);
                } else {
                    self.in_gallery.insert(name.to_string());
                }
            }
            "L" => {
                if let Some(_) = room {
                    if !self.in_rooms.contains_key(name) {
                        return Err(());
                    }
                    self.in_rooms.remove(name);
                } else {
                    if self.in_rooms.contains_key(name) {
                        return Err(());
                    }
                    self.in_gallery.remove(name);
                }
            }
            _ => return Err(())
        }
        Ok(())
    }
}