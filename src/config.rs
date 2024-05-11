#[derive(Debug, Deserialize)]
struct Configure {
    is_controller: bool,
    seed_server_address: Optional<String>,
}