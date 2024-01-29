pub struct Envs {
    pub db_loader_host: String,
    pub db_loader_port: u16,
    pub mode: String,
}
impl Envs {
    pub fn parse() -> Envs {
        Envs {
            db_loader_host: envmnt::get_or("DB_LOADER_HOST", "[::]"),
            db_loader_port: envmnt::get_or("DB_LOADER_PORT", "8002").parse().unwrap(),
            mode: envmnt::get_or("MODE", "info"),
        }
    }
}
