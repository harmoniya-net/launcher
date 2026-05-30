/// Group color/name table — mirrors src/server/api/account/group.get.ts.
pub struct Group {
    pub name: &'static str,
    pub color: u32,
}

pub fn lookup(modpack_title: &str) -> Option<Group> {
    match modpack_title.to_lowercase().as_str() {
        "wizard" => Some(Group { name: "Гравець", color: 0x8d8a93 }),
        "technomagic" => Some(Group { name: "Омеґа", color: 0xe85d5d }),
        "arcanetech" => Some(Group { name: "Мандрівник", color: 0x8d8a93 }),
        _ => None,
    }
}
