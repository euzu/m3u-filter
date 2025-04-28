export interface ServerStatus {
    status: string,
    version: string,
    build_time: string,
    server_time: string,
    memory: string,
    cache: string,
    active_user: number,
    active_user_connections: number,
    active_provider_connections: Record<string, number>,
}
