pub struct Envs {
    pub host: String,
    pub port: u16,
    pub db_loader_host: String,
    pub db_loader_port: u16,
    pub mode: String,
}
impl Envs {
    pub fn parse() -> Envs {
        Envs {
            host: envmnt::get_or("HOST", "127.0.0.1"),
            port: envmnt::get_or("PORT", "8000").parse().unwrap(),
            db_loader_host: envmnt::get_or("DB_LOADER_HOST", "[::]"),
            db_loader_port: envmnt::get_or("DB_LOADER_PORT", "8002").parse().unwrap(),
            mode: envmnt::get_or("MODE", "info"),
        }
    }
}
