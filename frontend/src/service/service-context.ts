import ConfigService from "./config-service";
import PlaylistService from "./playlist-service";
import FileService from "./file-service";
import AuthService from "./auth-service";
import UserConfigService from "./user-config-service";
import ServerStatusService from "./server-status-service";

export interface Services {
    config(): ConfigService;

    playlist(): PlaylistService;

    file(): FileService;

    auth(): AuthService;

    userConfig(): UserConfigService;

    status(): ServerStatusService;
}

class ServiceContextImpl implements Services {

    private readonly _configService: ConfigService = new ConfigService();
    private readonly _playlistService: PlaylistService = new PlaylistService();
    private readonly _fileService: FileService = new FileService();
    private readonly _authService: AuthService = new AuthService();
    private readonly _userConfigService: UserConfigService = new UserConfigService();
    private readonly _statusService: ServerStatusService = new ServerStatusService();

    config() {
        return this._configService;
    }

    playlist() {
        return this._playlistService;
    }

    file() {
        return this._fileService;
    }

    auth() {
        return this._authService;
    }

    userConfig(): UserConfigService {
        return this._userConfigService;
    }

    status(): ServerStatusService {
        return this._statusService;
    }
}

const ServiceContext = new ServiceContextImpl();
export default ServiceContext;

